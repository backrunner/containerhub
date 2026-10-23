use containerhub_core::fake::FakeProvider;
use containerhub_core::*;

fn service(mode: SwarmMode, replicas: Option<u64>) -> StackService {
    StackService {
        name: "shop_web".into(),
        containers: vec![],
        swarm: Some(SwarmService {
            key: ResourceKey {
                environment: "docker:test".into(),
                provider: ProviderKind::Docker,
                namespace: Some("swarm-service".into()),
                id: "shop_web".into(),
                incarnation: "a".repeat(25),
            },
            stack: "shop".into(),
            version: 12,
            image: "nginx:alpine".into(),
            mode,
            desired_replicas: replicas,
            force_update: 0,
            update_state: Some("completed".into()),
            ports: vec![],
            tasks_truncated: false,
            tasks: vec![SwarmTask {
                container_id: None,
                id: "b".repeat(25),
                node: "node".into(),
                slot: 1,
                state: "running".into(),
                desired_state: "running".into(),
            }],
        }),
    }
}
#[test]
fn deployment_state_does_not_equate_update_completed_with_convergence() {
    let mut s = service(SwarmMode::Replicated, Some(3));
    assert_eq!(s.status(), StackServiceStatus::Scheduling);
    s.swarm.as_mut().unwrap().desired_replicas = Some(1);
    assert_eq!(s.status(), StackServiceStatus::Running);
    // Failed historical tasks must not turn a converged service red.
    s.swarm.as_mut().unwrap().tasks.push(SwarmTask {
        container_id: None,
        id: "c".repeat(25),
        node: "node".into(),
        slot: 1,
        state: "failed".into(),
        desired_state: "shutdown".into(),
    });
    assert_eq!(s.status(), StackServiceStatus::Running);
    s.swarm.as_mut().unwrap().tasks_truncated = true;
    assert_eq!(s.status(), StackServiceStatus::Incomplete);
    s.swarm.as_mut().unwrap().update_state = Some("rollback_paused".into());
    assert_eq!(s.status(), StackServiceStatus::UpdatePaused);
}
#[test]
fn zero_replicas_and_global_inventory_remain_distinct() {
    let mut s = service(SwarmMode::Replicated, Some(0));
    assert_eq!(s.status(), StackServiceStatus::Updating);
    s.swarm.as_mut().unwrap().tasks.clear();
    assert_eq!(s.status(), StackServiceStatus::Stopped);
    s.swarm.as_mut().unwrap().mode = SwarmMode::Global;
    s.swarm.as_mut().unwrap().desired_replicas = None;
    assert_eq!(s.status(), StackServiceStatus::Unknown);
}
#[test]
fn service_scope_never_broadens_to_other_members_or_same_named_projects() {
    let first = service(SwarmMode::Replicated, Some(1));
    let mut second = service(SwarmMode::Global, None);
    second.name = "shop_agent".into();
    let s = second.swarm.as_mut().unwrap();
    s.key.id = second.name.clone();
    s.key.incarnation = "b".repeat(25);
    let stack = Stack {
        environment: "docker:test".into(),
        name: "shop".into(),
        kind: StackKind::Swarm,
        services: vec![first, second],
    };
    assert!(
        stack.commands(StackAction::Stop).is_err(),
        "global cannot scale to zero"
    );
    let commands = stack
        .commands_for_services(StackAction::Stop, &["shop_web".into()])
        .unwrap();
    assert_eq!(commands.len(), 1);
    assert!(
        matches!(&commands[0], Command::SwarmService { key, action: SwarmAction::Scale { replicas: 0 }, .. } if key.id == "shop_web")
    );
    for names in [
        vec![],
        vec!["missing".into()],
        vec!["shop_web".into(), "shop_web".into()],
    ] {
        assert!(
            stack
                .commands_for_services(StackAction::Restart, &names)
                .is_err()
        );
    }
    let mut foreign = stack;
    foreign.services[0].swarm.as_mut().unwrap().key.environment = "docker:other".into();
    assert!(
        foreign
            .commands_for_services(StackAction::Restart, &["shop_web".into()])
            .is_err()
    );
}
#[tokio::test]
async fn compose_health_is_separate_and_one_off_jobs_are_not_lifecycle_members() {
    let mut snapshot = FakeProvider::default().snapshot().await.unwrap();
    snapshot.capabilities.stacks = true;
    for c in &mut snapshot.containers {
        c.key.provider = ProviderKind::Docker;
        c.compose = Some(ComposeMembership {
            project: "shop".into(),
            service: "web".into(),
            one_off: false,
        });
        c.state = ContainerState::Running;
        c.health = ContainerHealth::Unconfigured;
    }
    let mut stack = Stack::from_snapshot(&snapshot).remove(0);
    let s = &mut stack.services[0];
    assert_eq!(s.status(), StackServiceStatus::Running);
    s.containers[0].health = ContainerHealth::Unhealthy;
    assert_eq!(s.status(), StackServiceStatus::Unhealthy);
    s.containers[0].compose.as_mut().unwrap().one_off = true;
    assert_eq!(s.status(), StackServiceStatus::Running);
    assert_eq!(
        stack
            .commands_for_services(StackAction::Restart, &["web".into()])
            .unwrap()
            .len(),
        snapshot.containers.len() - 1
    );
}

#[tokio::test]
async fn container_overview_uses_real_ids_and_preserves_remote_unknowns() {
    let mut snapshot = FakeProvider::default().snapshot().await.unwrap();
    snapshot.environment = "docker:test".into();
    snapshot.capabilities.stacks = true;
    let mut s = service(SwarmMode::Replicated, Some(3)).swarm.unwrap();
    let mut local = snapshot.containers.remove(0);
    local.key.environment = snapshot.environment.clone();
    local.key.provider = ProviderKind::Docker;
    local.key.incarnation = "1".repeat(64);
    local.health = ContainerHealth::Unhealthy;
    let mut foreign = local.clone();
    foreign.key.environment = "other".into();
    foreign.key.incarnation = "2".repeat(64);
    snapshot.containers = vec![local, foreign];
    s.tasks[0].container_id = Some("1".repeat(64));
    let mut remote = s.tasks[0].clone();
    remote.id = "remote".into();
    remote.slot = 2;
    remote.container_id = Some("2".repeat(64));
    s.tasks.push(remote.clone());
    s.tasks.push(remote); // Duplicate task metadata is one container, not two.
    let mut history = s.tasks[0].clone();
    history.desired_state = "shutdown".into();
    history.container_id = Some("3".repeat(64));
    s.tasks.push(history);
    let mut pending = s.tasks[0].clone();
    pending.state = "pending".into();
    pending.container_id = None;
    s.tasks.push(pending.clone());
    pending.container_id = Some("not-a-container-id".into());
    s.tasks.push(pending);
    snapshot.swarm = SwarmInventory {
        status: SwarmStatus::Manager,
        services: vec![s],
    };
    let stacks = Stack::from_snapshot(&snapshot);
    let overview = stacks[0].services[0].container_overview();
    assert_eq!(overview.len(), 2);
    assert_eq!(
        overview[0].inspected.unwrap().health,
        ContainerHealth::Unhealthy
    );
    assert_eq!(overview[1].id, "2".repeat(64));
    assert!(
        overview[1].inspected.is_none(),
        "foreign environment must not supply inspection"
    );
    // Group lifecycle still addresses Swarm services, never member containers.
    assert!(matches!(
        &stacks[0].commands(StackAction::Restart).unwrap()[0],
        Command::SwarmService { .. }
    ));
}

#[test]
fn older_agent_task_without_container_identity_stays_unknown() {
    let task: SwarmTask = serde_json::from_str(
        r#"{"id":"task","node":"node","slot":1,"state":"running","desired_state":"running"}"#,
    )
    .unwrap();
    assert!(task.container_id.is_none());
    let mut s = service(SwarmMode::Replicated, Some(1));
    s.swarm.as_mut().unwrap().tasks = vec![task];
    assert!(s.container_overview().is_empty());
    assert_eq!(s.running_count(), 1);
}
