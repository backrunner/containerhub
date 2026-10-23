mod swarm;
use super::*;
use serde_json::{Value, json};
use std::os::unix::fs::PermissionsExt;
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "containerhub-docker-{}-{}-{}",
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("docker"),
            include_str!("../../../../tests/fixtures/docker/cli_fixture.py"),
        )
        .unwrap();
        std::fs::set_permissions(root.join("docker"), std::fs::Permissions::from_mode(0o700))
            .unwrap();
        let f = Self(root);
        f.write(json!({"containers":[record(1,"api","running"),record(2,"db","exited")]}));
        f
    }
    fn write(&self, v: Value) {
        std::fs::write(self.0.join("state.json"), serde_json::to_vec(&v).unwrap()).unwrap();
    }
    fn read(&self) -> Value {
        serde_json::from_slice(&std::fs::read(self.0.join("state.json")).unwrap()).unwrap()
    }
    fn provider(&self) -> DockerProvider {
        DockerProvider::new("fixture", Some(&self.0.join("docker"))).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn record(id: u32, service: &str, status: &str) -> Value {
    json!({
        "Id":format!("{id:064x}"),"Name":format!("/shop-{service}-1"),"Platform":"linux","Created":"2026-10-23T00:00:00Z",
        "Config":{"Image":"example.invalid/app:v1","Env":["PASSWORD=private-secret"],"Labels":{"com.docker.compose.project":"shop","com.docker.compose.service":service,"private.label":"private-secret","com.docker.compose.project.config_files":"/private/do-not-execute.yml"}},
        "State":{"Status":status,"Health":{"Status":"healthy"}},"HostConfig":{"Memory":536870912,"NanoCpus":2000000000,"AutoRemove":false},"NetworkSettings":{"Ports":{"3000/tcp":[{"HostIp":"127.0.0.1","HostPort":"3000"}]},"Networks":{"shop_default":{"IPAddress":"172.17.0.2"}}}
    })
}
#[tokio::test]
async fn compose_inventory_actions_and_identity_revalidation() {
    let f = Fixture::new();
    let p = f.provider();
    let s = p.snapshot().await.unwrap();
    assert!(s.capabilities.stacks);
    let text = serde_json::to_string(&s).unwrap();
    assert!(!text.contains("private-secret"));
    assert!(!text.contains("do-not-execute"));
    let stacks = Stack::from_snapshot(&s);
    assert_eq!(stacks.len(), 1);
    assert_eq!(stacks[0].services.len(), 2);
    let start = stacks[0].commands(StackAction::Start).unwrap();
    assert_eq!(start.len(), 1);
    p.execute(&start[0]).await.unwrap();
    assert!(
        p.snapshot()
            .await
            .unwrap()
            .containers
            .iter()
            .all(|c| c.state.is_running())
    );
    let api = s.containers[0].key.clone();
    p.lifecycle(&api, LifecycleAction::Stop).await.unwrap();
    assert_eq!(
        p.current(&api).await.unwrap().0.state,
        ContainerState::Stopped
    );
    let mut state = f.read();
    state["containers"][0]["Id"] = json!(format!("{:064x}", 99));
    f.write(state);
    assert!(p.lifecycle(&api, LifecycleAction::Start).await.is_err());
    let calls = std::fs::read_to_string(f.0.join("arguments.jsonl")).unwrap();
    assert!(!calls.contains("/private/do-not-execute.yml"));
    assert!(!calls.contains("compose"));
    let mut state = f.read();
    state["engine"] = json!("changed-engine");
    f.write(state);
    assert_eq!(p.snapshot().await.unwrap_err().kind, ErrorKind::Conflict);
}
#[tokio::test]
async fn delete_is_stopped_only_and_preserves_other_resources() {
    let f = Fixture::new();
    let p = f.provider();
    let s = p.snapshot().await.unwrap();
    assert_eq!(
        p.execute(&Command::DeleteContainer {
            key: s.containers[0].key.clone()
        })
        .await
        .unwrap_err()
        .kind,
        ErrorKind::Conflict
    );
    p.execute(&Command::DeleteContainer {
        key: s.containers[1].key.clone(),
    })
    .await
    .unwrap();
    assert_eq!(p.snapshot().await.unwrap().containers.len(), 1);
    let calls = std::fs::read_to_string(f.0.join("arguments.jsonl")).unwrap();
    assert!(!calls.contains("--volumes"));
    assert!(!calls.contains("--force"));
}
#[tokio::test]
async fn creation_returns_immutable_identity_and_rejects_unsupported_options() {
    let f = Fixture::new();
    let p = f.provider();
    let mut c = Command::CreateContainer {
        environment: p.environment.clone(),
        spec: CreateContainer {
            name: "fresh".into(),
            image: "alpine:3.22".into(),
            cpus: 1,
            memory_mib: 256,
            ports: vec![],
            options: Default::default(),
        },
    };
    let CommandResult::Container(created) = p.execute(&c).await.unwrap() else {
        panic!()
    };
    assert_eq!(created.key.id, "fresh");
    assert_eq!(created.key.incarnation.len(), 64);
    if let Command::CreateContainer { spec, .. } = &mut c {
        spec.options.read_only = true;
    }
    assert_eq!(
        p.execute(&c).await.unwrap_err().kind,
        ErrorKind::Unsupported
    );
    let mut state = f.read();
    state["offline"] = json!(true);
    f.write(state);
    let error = p.snapshot().await.unwrap_err();
    assert!(!error.message.contains("token-do-not-leak"));
}
#[tokio::test]
async fn stack_plans_exclude_one_off_jobs_and_foreign_namespaces() {
    let f = Fixture::new();
    let p = f.provider();
    let mut s = p.snapshot().await.unwrap();
    s.containers[1].compose.as_mut().unwrap().one_off = true;
    assert!(
        Stack::from_snapshot(&s)[0]
            .commands(StackAction::Start)
            .unwrap()
            .is_empty()
    );
    s.containers[1].key.environment = "foreign".into();
    assert_eq!(Stack::from_snapshot(&s)[0].containers().len(), 1);
    s.containers[0].compose.as_mut().unwrap().project = "../unsafe".into();
    assert!(Stack::from_snapshot(&s).is_empty());
}
