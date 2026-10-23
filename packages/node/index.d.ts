export type ProviderKind = 'apple' | 'docker' | 'kubernetes';
export type LifecycleAction = 'Start' | 'Stop' | 'Restart';
export type ResourceKind = 'Image' | 'Volume' | 'Network';
export interface ResourceKey { environment: string; provider: ProviderKind; namespace: string | null; id: string; incarnation: string }
export interface Port { host: string; host_port: number; container_port: number; protocol: 'tcp' | 'udp' }
export interface VolumeMount { name: string; destination: string; read_only?: boolean }
/** Literal argv; ordinary task history stores this configuration. */
export interface StartupCommand { entrypoint?: string; arguments?: string[] }
export interface CreateOptions { start_after_create?: boolean; startup?: StartupCommand; secret_environment?: string[]; networks?: string[]; volumes?: VolumeMount[]; user?: string; working_directory?: string; read_only?: boolean }
export type OperationProgress = 'waiting' | 'fetching' | 'unpacking' | 'verifying' | 'stopping' | 'replacing' | 'creating' | 'starting';
export interface ContainerDetails { container: Container; observed_at: number; read_only: boolean | null; working_directory: string | null; user: string | null; environment_names: string[]; label_names: string[]; argument_count: number }
export interface CreateContainer { name: string; image: string; cpus: number; memory_mib: number; ports?: Port[]; options?: CreateOptions }
export type Command =
  | { kind: 'swarm_service'; key: ResourceKey; stack: string; version: number; action: SwarmAction }
  | { kind: 'start_runtime'; environment: string }
  | { kind: 'lifecycle'; key: ResourceKey; action: LifecycleAction }
  | { kind: 'pull_image'; environment: string; reference: string }
  | { kind: 'create_container'; environment: string; spec: CreateContainer }
  | { kind: 'rebuild_ports'; key: ResourceKey; expected_state: 'Running' | 'Stopped'; expected: Port[]; ports: Port[] }
  | { kind: 'delete_container'; key: ResourceKey }
  | { kind: 'delete_image'; key: ImageKey };
export interface ImageKey { environment: string; provider: ProviderKind; reference: string; digest: string }
export interface ImageVariant { platform: string; digest: string; size_bytes: number | null; environment_names: string[] }
export interface ImageDetails { key: ImageKey; observed_at: number; created_at: string | null; media_type: string | null; variants: ImageVariant[]; used_by: ResourceKey[]; other_references: string[] }
export interface LogExport { file_name: string; content: string; line_count: number; snapshot_lines: number; truncated: boolean }
export interface Container { compose: ComposeMembership | null; health: ContainerHealth; key: ResourceKey; image: string; state: 'Running' | 'Stopped' | { Unknown: string }; cpus: number | null; memory_limit: number | null; platform: string; ports: Port[]; mounts: string[]; networks: string[]; addresses: string[]; environment_names: string[]; label_names: string[]; created_at: string; auto_remove: boolean | null }
export interface Capabilities { stacks: boolean; rebuild_ports: boolean; inspect_container: boolean; cancel_pull: boolean; inspect_image: boolean; delete_image: boolean; pull_image: boolean; create_container: boolean; create_advanced: boolean; create_secrets: boolean; create_startup: boolean; create_and_start: boolean; delete_container: boolean; stats: boolean }
export type RuntimeFeature = 'stacks' | 'start_runtime' | 'containers' | 'images' | 'volumes' | 'networks' | 'logs' | 'boot_logs' | 'inspect_container' | 'inspect_image' | 'stats' | 'lifecycle' | 'pull_image' | 'cancel_pull' | 'create_container' | 'startup_command' | 'create_and_start' | 'secret_environment' | 'advanced_create' | 'delete_container' | 'delete_image' | 'rebuild_ports';
export interface CapabilityCheck { feature: RuntimeFeature; available: boolean; reason: string | null }
export interface RuntimeDiagnostics { environment: string; cli_path: string | null; cli_version: string | null; observed_at: number; service_error: { kind: string; message: string } | null; checks: CapabilityCheck[] }
export interface Snapshot { swarm: SwarmInventory; environment: string; cli_version: string; cli_path: string; observed_at: number; containers: Container[]; lifecycle_supported: boolean; capability_note: string | null; capabilities: Capabilities }
export interface Resource { id: string; name: string; kind: ResourceKind; detail: string; size: number | null }
export interface Stats { key: ResourceKey; observed_at: number; cpu_percent: number | null; memory_bytes: number | null; memory_limit_bytes: number | null }
export interface Logs { key: ResourceKey; source: 'Standard' | 'Boot'; observed_at: number; lines: string[]; truncated: boolean }
export type CommandResult = { kind: 'confirmed' } | { kind: 'image'; value: Resource } | { kind: 'container'; value: Container };
export type ExecutionReceipt = { kind: 'remote'; agent: string; owner: string; request_id: string; expires_at: number };
export interface Operation { receipt: ExecutionReceipt | null; progress: OperationProgress | null; cancel_requested: boolean; id: number; command: Command; result: CommandResult | null; state: 'Submitting' | 'Queued' | 'Running' | 'Reconciling' | 'Succeeded' | 'Failed' | 'Cancelled' | 'UnknownOutcome'; started_at: number; finished_at: number | null; message: string | null }
export type ConnectionOptions = { binary: string; stateDirectory: string } & ({ mode: 'docker'; dockerContext: string; dockerBinary?: string; appleBinary?: never; profile?: never; agent?: never } | { mode: 'apple'; appleBinary?: string; profile?: never; agent?: never } | { mode: 'demo'; appleBinary?: never; profile?: never; agent?: never } | { mode: 'remote'; appleBinary?: never; profile: string; agent?: string });
export interface WaitOptions { timeoutMs?: number; signal?: AbortSignal; onProgress?: (operation: Operation) => void }
export class ContainerHubError extends Error { kind: string; operationId?: number }
export class ContainerHub {
  private constructor(options: ConnectionOptions);
  static connect(options: ConnectionOptions): Promise<ContainerHub>;
  hostSnapshot(): Promise<HostSnapshot>;
  diagnostics(): Promise<RuntimeDiagnostics>;
  stacks(): Promise<Stack[]>;
  stackCommands(environment: string, name: string, action: StackAction, kind?: StackKind): Promise<Command[]>;
  snapshot(): Promise<Snapshot>;
  resources(kind: ResourceKind): Promise<Resource[]>;
  inspectContainer(key: ResourceKey): Promise<ContainerDetails>;
  cancel(id: number): Promise<Operation>;
  reconcile(id: number): Promise<Operation>;
  inspectImage(environment: string, reference: string): Promise<ImageDetails>;
  exportLogs(key: ResourceKey, options?: { source?: 'Standard' | 'Boot'; query?: string }): Promise<LogExport>;
  logs(key: ResourceKey, source?: 'Standard' | 'Boot'): Promise<Logs>;
  stats(key: ResourceKey): Promise<Stats>;
  operation(id: number): Promise<Operation>;
  operations(): Promise<Operation[]>;
  submit(command: Command, options?: { operationId?: number; secrets?: Record<string, string> }): Promise<Operation>;
  wait(operationId: number, options?: WaitOptions): Promise<Operation>;
  execute(command: Command, options?: WaitOptions & { operationId?: number; secrets?: Record<string, string> }): Promise<Operation>;
  close(options?: { timeoutMs?: number }): Promise<void>;
}

export type ContainerHealth = 'healthy' | 'unhealthy' | 'starting' | 'unconfigured' | 'unsupported' | 'unknown';
export interface HostSnapshot {
  environment: string; observed_at: number; hostname: string | null; os: string | null;
  architecture: string; processor: string | null; logical_cpus: number; uptime_seconds: number;
  cpu_percent: number | null; memory_used_bytes: number | null; memory_total_bytes: number | null;
  disk_path: string | null; disk_used_bytes: number | null; disk_total_bytes: number | null;
  disk_read_bytes_per_second: number | null; disk_write_bytes_per_second: number | null;
  network_receive_bytes_per_second: number | null; network_send_bytes_per_second: number | null;
  sample: boolean;
}

export interface ComposeMembership { project: string; service: string; one_off: boolean }
export type StackKind = 'compose' | 'swarm';
export interface Stack { kind: StackKind; environment: string; name: string; services: { name: string; containers: Container[]; swarm: SwarmService | null }[] }
export type StackAction = 'start' | 'stop' | 'restart' | 'remove_stopped' | 'remove_services';

export type SwarmAction = { kind: 'scale'; replicas: number } | { kind: 'restart' } | { kind: 'remove' };
export interface SwarmInventory { status: 'inactive' | 'worker' | 'manager' | 'unavailable'; services: SwarmService[] }
export interface SwarmService { key: ResourceKey; stack: string; version: number; image: string; mode: 'replicated' | 'global' | 'job' | 'unknown'; desired_replicas: number | null; force_update: number; update_state: string | null; ports: string[]; tasks: SwarmTask[]; tasks_truncated: boolean }
export interface SwarmTask { id: string; node: string; slot: number; desired_state: string; state: string; container_id?: string | null }
