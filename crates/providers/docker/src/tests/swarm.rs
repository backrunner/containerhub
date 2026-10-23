use super::*;
use containerhub_core::environment::EnvironmentClient;
fn service(id: &str, name: &str, global: bool) -> Value {
    json!({"ID":id,"Version":{"Index":12},"Spec":{"Name":name,"Labels":{"com.docker.stack.namespace":"shop","private":"secret-do-not-leak"},"Mode":if global{json!({"Global":{}})}else{json!({"Replicated":{"Replicas":3}})},"TaskTemplate":{"ForceUpdate":0,"ContainerSpec":{"Image":"nginx:alpine","Env":["TOKEN=secret-do-not-leak"]}}},"Endpoint":{"Ports":[{"PublishedPort":8080,"TargetPort":80,"Protocol":"tcp"}]}})
}
fn swarm(f: &Fixture) {
    let mut state = f.read();
    state["swarm"] = json!({"LocalNodeState":"active","ControlAvailable":true});
    state["services"] = json!([
        service("aaaaaaaaaaaaaaaaaaaaaaaaa", "shop_web", false),
        service("bbbbbbbbbbbbbbbbbbbbbbbbb", "shop_agent", true)
    ]);
    state["tasks"] = json!([{"ID":"ccccccccccccccccccccccccc","ServiceID":"aaaaaaaaaaaaaaaaaaaaaaaaa","NodeID":"ddddddddddddddddddddddddd","Slot":1,"DesiredState":"running","Status":{"State":"running","ContainerStatus":{"ContainerID":format!("{:064x}",1)},"Err":"secret-do-not-leak"}}]);
    f.write(state);
}
#[tokio::test]
async fn swarm_inventory_scaling_restart_and_removal_use_service_ids() {
    let f = Fixture::new();
    swarm(&f);
    let p = f.provider();
    let snapshot = p.snapshot().await.unwrap();
    assert_eq!(snapshot.swarm.status, SwarmStatus::Manager);
    assert_eq!(snapshot.swarm.services[0].running(), 1);
    assert!(
        !serde_json::to_string(&snapshot)
            .unwrap()
            .contains("secret-do-not-leak")
    );
    let stacks = Stack::from_snapshot(&snapshot);
    assert_eq!(stacks.len(), 2);
    assert_ne!(stacks[0].identity(), stacks[1].identity());
    let overview = stacks[1].services[0].container_overview();
    assert_eq!(overview.len(), 1);
    assert_eq!(overview[0].id, format!("{:064x}", 1));
    assert_eq!(overview[0].inspected.unwrap().key.id, "shop-api-1");
    let s = &snapshot.swarm.services[0];
    p.execute(&s.command(SwarmAction::Scale { replicas: 0 }).unwrap())
        .await
        .unwrap();
    assert_eq!(
        p.snapshot().await.unwrap().swarm.services[0].desired_replicas,
        Some(0)
    );
    assert_eq!(
        p.execute(&s.command(SwarmAction::Restart).unwrap())
            .await
            .unwrap_err()
            .kind,
        ErrorKind::Conflict
    );
    let fresh = p.snapshot().await.unwrap().swarm.services.remove(0);
    p.execute(&fresh.command(SwarmAction::Restart).unwrap())
        .await
        .unwrap();
    let fresh = p.snapshot().await.unwrap().swarm.services.remove(0);
    assert_eq!(fresh.force_update, 1);
    p.execute(&fresh.command(SwarmAction::Remove).unwrap())
        .await
        .unwrap();
    assert_eq!(p.snapshot().await.unwrap().swarm.services.len(), 1);
    let calls = std::fs::read_to_string(f.0.join("arguments.jsonl")).unwrap();
    assert!(!calls.contains("network\", \"rm"));
    assert!(!calls.contains("--volumes"));
    assert_eq!(
        snapshot.swarm.services[1]
            .command(SwarmAction::Scale { replicas: 0 })
            .unwrap_err()
            .kind,
        ErrorKind::Unsupported
    );
    assert!(stacks[1].commands(StackAction::Stop).is_err());
}
#[tokio::test]
async fn worker_unavailable_replaced_and_foreign_services_are_not_mutated() {
    let f = Fixture::new();
    swarm(&f);
    let p = f.provider();
    let s = p.snapshot().await.unwrap().swarm.services.remove(0);
    let mut state = f.read();
    state["swarm"]["ControlAvailable"] = json!(false);
    f.write(state);
    assert_eq!(
        p.snapshot().await.unwrap().swarm.status,
        SwarmStatus::Worker
    );
    assert_eq!(
        p.execute(&s.command(SwarmAction::Restart).unwrap())
            .await
            .unwrap_err()
            .kind,
        ErrorKind::Unsupported
    );
    swarm(&f);
    let mut state = f.read();
    state["swarm_fail"] = json!(true);
    f.write(state);
    assert_eq!(
        p.snapshot().await.unwrap().swarm.status,
        SwarmStatus::Unavailable
    );
    let mut state = f.read();
    state["swarm_fail"] = json!(false);
    state["services"][0]["ID"] = json!("zzzzzzzzzzzzzzzzzzzzzzzzz");
    f.write(state);
    assert!(
        p.execute(&s.command(SwarmAction::Restart).unwrap())
            .await
            .is_err()
    );
    let mut s = s;
    s.key.environment = "foreign".into();
    assert_eq!(
        p.execute(&s.command(SwarmAction::Remove).unwrap())
            .await
            .unwrap_err()
            .kind,
        ErrorKind::Conflict
    );
    let calls = std::fs::read_to_string(f.0.join("arguments.jsonl")).unwrap();
    assert!(!calls.contains("\"update\""));
    assert!(!calls.contains("\"rm\""));
}
#[tokio::test]
async fn swarm_identity_maps_through_remote_environment_adapter() {
    let f = Fixture::new();
    swarm(&f);
    let p = f.provider();
    let local = p.environment.clone();
    let remote = EnvironmentClient::new(
        std::sync::Arc::new(p),
        local,
        "agent:docker".into(),
        ProviderKind::Docker,
    );
    let s = remote.snapshot().await.unwrap().swarm.services.remove(0);
    assert_eq!(s.key.environment, "agent:docker");
    remote
        .execute(&s.command(SwarmAction::Scale { replicas: 2 }).unwrap())
        .await
        .unwrap();
    assert_eq!(
        remote.snapshot().await.unwrap().swarm.services[0].desired_replicas,
        Some(2)
    );
}

#[tokio::test]
async fn swarm_task_containers_cannot_be_started_as_compose_members() {
    let f = Fixture::new();
    let mut state = f.read();
    state["containers"][0]["Config"]["Labels"]["com.docker.swarm.service.id"] =
        json!("a".repeat(25));
    f.write(state);
    let p = f.provider();
    let s = p.snapshot().await.unwrap();
    assert!(s.containers[0].compose.is_none());
    assert_eq!(
        p.lifecycle(&s.containers[0].key, LifecycleAction::Stop)
            .await
            .unwrap_err()
            .kind,
        ErrorKind::Unsupported
    );
    assert_eq!(Stack::from_snapshot(&s)[0].containers().len(), 1);
}

#[tokio::test]
async fn successful_remove_output_does_not_confirm_a_service_that_still_exists() {
    let f = Fixture::new();
    swarm(&f);
    let p = f.provider();
    let s = p.snapshot().await.unwrap().swarm.services.remove(0);
    assert_eq!(s.key.incarnation.len(), 25);
    let mut state = f.read();
    state["ignore_remove"] = json!(true);
    f.write(state);
    assert_eq!(
        p.execute(&s.command(SwarmAction::Remove).unwrap())
            .await
            .unwrap_err()
            .kind,
        ErrorKind::UnknownOutcome
    );
}
