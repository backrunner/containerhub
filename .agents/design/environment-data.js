/* Synthetic review fixtures only. Nothing is read from a host or saved outside this page. */
let reviewEnvironment = 'demo';
let inspectedEnvironment = 'studio';
let environmentQuery = '';
let environmentRange = 60;
let environmentPaused = false;
const reviewCapabilities = ['启动运行时服务','容器列表','镜像列表','存储卷列表','网络列表','应用日志','启动日志','容器检查','镜像检查','实时统计','启动、停止与重启','拉取镜像','取消拉取','创建容器','高级创建选项','秘密环境变量','自定义启动命令','创建后立即启动','重建端口映射','删除容器','删除镜像'];
const reviewTargets = [
  {id:'local',name:'本机',description:'日常开发',transport:'local',host:'macbook-pro.local',hardware:'MacBook Pro',processor:'Apple M4 Pro',cores:12,memory:24,disk:512,diskUsed:216,os:'macOS 26.0',arch:'arm64',uptime:'3 天 6 小时',group:'本机',containers:{running:5,stopped:2,error:1,healthy:3,unhealthy:0,unchecked:2},cpu:16.8,mem:9.4,network:0.8,io:2.1,autoConnect:true,refresh:2},
  {id:'demo',name:'演示工作空间',description:'示例数据',transport:'local',host:'demo.local',hardware:'示例主机',processor:'Apple M4',cores:10,memory:16,disk:256,diskUsed:82,os:'macOS 26.0',arch:'arm64',uptime:'6 小时 24 分',group:'本机',containers:{running:5,stopped:2,error:1,healthy:3,unhealthy:1,unchecked:1},cpu:21.4,mem:6.2,network:0.5,io:1.4,autoConnect:true,refresh:2},
  {id:'studio',name:'开发工作站',description:'团队开发与集成测试',transport:'direct',policy:'auto',host:'studio-dev.local',hardware:'Mac mini',processor:'Apple M4 Pro',cores:12,memory:32,disk:512,diskUsed:142,os:'macOS 26.0',arch:'arm64',uptime:'12 天 18 小时',group:'远程',endpoint:'wss://192.168.31.42:7443',relay:'wss://relay-sg.example.invalid',latency:12,connected:'2 小时 16 分',agent:'0.1.0',paired:'2026-10-20 09:42',fingerprint:'p256:8e70f2161ab8482fa7246655b3947af8d20cf8a01bea4a6687d293b145e26c74',containers:{running:12,stopped:2,error:1,healthy:9,unhealthy:1,unchecked:2},cpu:24.6,mem:11.8,network:1.24,io:3.6,autoConnect:true,refresh:2},
  {id:'build',name:'构建节点',description:'新加坡 · 持续集成',transport:'relay',policy:'auto',host:'sg-build-01.local',hardware:'Mac Studio',processor:'Apple M4 Max',cores:16,memory:64,disk:1024,diskUsed:608,os:'macOS 26.0',arch:'arm64',uptime:'26 天 4 小时',group:'远程',endpoint:'wss://10.42.0.18:7443',relay:'wss://relay-sg.example.invalid',latency:48,connected:'8 小时 03 分',agent:'0.1.0',paired:'2026-10-18 15:20',fingerprint:'p256:b8cefa6cb1c244e2acd5f37c61eb8b37951428ea7af845e3b42222ff478b1846',containers:{running:8,stopped:4,error:0,healthy:6,unhealthy:0,unchecked:2},cpu:68.2,mem:38.4,network:7.2,io:24.8,autoConnect:true,refresh:2},
  {id:'qa',name:'测试节点',description:'隔离回归环境',transport:'direct',policy:'direct',host:'qa-mini.local',hardware:'Mac mini',processor:'Apple M2',cores:8,memory:16,disk:256,diskUsed:98,os:'macOS 26.0',arch:'arm64',uptime:'5 天 2 小时',group:'远程',endpoint:'wss://192.168.31.65:7443',relay:'',latency:18,connected:'—',agent:'0.1.0',paired:'2026-10-19 11:08',fingerprint:'p256:37c351caf9134ab8bc2fdfc261f6446ad9bb7b33bef842718f09bde21d01facf',containers:{running:3,stopped:3,error:0,healthy:2,unhealthy:0,unchecked:1},cpu:8.2,mem:4.6,network:0.2,io:0.5,offline:true,lastSeen:'8 分钟前',autoConnect:false,refresh:5},
];
reviewTargets.push({...reviewTargets[1],id:"docker",name:"Docker 开发",provider:"docker",context:"desktop-linux",description:"",host:"desktop-linux",containers:{running:6,stopped:1,error:0,healthy:3,unhealthy:1,unchecked:2}});
const envTarget = id => reviewTargets.find(t=>t.id===id);
const inspectedTarget = () => envTarget(inspectedEnvironment) || reviewTargets[0];
const environmentIsOffline = t => !!t.offline || (isOffline() && t.id===inspectedEnvironment);
const transportLabel = t => t.provider==='docker'?'Docker Engine':t.transport==='local'?'本机':t.transport==='relay'?'中继连接':'局域网直连';
function environmentRows(t) {
  const c=t.containers, rows=[];
  for(let i=0;i<c.running;i++) rows.push({name:i===0&&c.unhealthy?'api-worker':i===1?'web-api':i===2?'postgres':`service-${String(i+1).padStart(2,'0')}`,state:'running',health:i<c.unhealthy?'unhealthy':i<c.unhealthy+c.healthy?'healthy':'unchecked'});
  for(let i=0;i<c.stopped;i++) rows.push({name:i===0?'migration':i===1?'sandbox':`maintenance-${i+1}`,state:'stopped',health:'excluded'});
  for(let i=0;i<c.error;i++) rows.push({name:'mailpit',state:'error',health:'excluded'});
  return rows;
}

reviewCapabilities.push("Stack 管理");
