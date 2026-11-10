(function(){
  const $ = (s, r) => (r || document).querySelector(s);
  const $$ = (s, r) => Array.from((r || document).querySelectorAll(s));
  const esc = s => String(s).replace(/[&<>"]/g, m => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;'}[m]));
  const ICON = n => '<svg class="icon"><use href="#i-' + n + '"/></svg>';

  const sidebar = $('#sidebar'), sidebarBackdrop = $('#sidebarBackdrop');
  const menuToggle = $('#menuToggle'), menuPop = $('#menuPop'), menuScrim = $('#menuScrim');
  const main = $('#main'), toast = $('#toast');
  const views = $$('.view');
  const inspector = $('#inspector'), inspectorScrim = $('#inspectorScrim'), inspectorBody = $('#inspectorBody');
  const rowMenu = $('#rowMenu');

  let toastTimer;
  function showToast(msg){
    toast.textContent = msg;
    toast.classList.add('show');
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => toast.classList.remove('show'), 2200);
  }

  /* ---------------------------------------------------------------- data */
  const MIN = 60, HOUR = 3600, DAY = 86400;
  const fmtAge = s => s < MIN ? s + 's' : s < HOUR ? Math.floor(s / MIN) + 'm' : s < DAY ? Math.floor(s / HOUR) + 'h' : Math.floor(s / DAY) + 'd';
  const pod = (name, ns, ctrs, restarts, ctrl, node, qos, age, status, ip) => ({id:'pod/' + ns + '/' + name, name, ns, ctrs, restarts, ctrl, node, qos, age, status, ip});

  const DATA = {
    pods: [
      pod('checkout-api-7c9f8d6b4f-x2k9m', 'default', ['api', 'envoy'], 0, 'Deployment/checkout-api', 'ip-10-0-4-18', 'Burstable', 2 * DAY, 'Running', '10.244.4.18'),
      pod('coredns-5c98db65d4-6mh2p', 'kube-system', ['coredns'], 0, 'Deployment/coredns', 'ip-10-0-4-2', 'Guaranteed', 12 * DAY, 'Running', '10.244.4.2'),
      pod('prometheus-server-0', 'monitoring', ['prometheus', 'config-reloader'], 0, 'StatefulSet/prometheus-server', 'ip-10-0-5-11', 'Guaranteed', 9 * DAY, 'Running', '10.244.5.11'),
      pod('payments-worker-6b8d9c7f5d-9nq4r', 'default', ['worker'], 14, 'Deployment/payments-worker', 'ip-10-0-4-19', 'Burstable', 6 * HOUR, 'CrashLoopBackOff', '10.244.4.31'),
      pod('argocd-repo-server-55b7c44d9c-7b2wm', 'argocd', ['repo-server'], 1, 'Deployment/argocd-repo-server', 'ip-10-0-5-8', 'Guaranteed', 18 * DAY, 'Running', '10.244.5.8'),
      pod('nginx-ingress-controller-5d9f8c6b7-x4z1p', 'default', ['controller'], 0, 'DaemonSet/nginx-ingress', 'ip-10-0-4-12', 'Guaranteed', 4 * DAY, 'Running', '10.244.4.12'),
      pod('grafana-7f6d4c8b5d-mz8kt', 'monitoring', ['grafana', 'sidecar'], 2, 'Deployment/grafana', 'ip-10-0-5-14', 'Burstable', 11 * DAY, 'Running', '10.244.5.14'),
      pod('cert-manager-webhook-6d5c9b7f4-qk2lp', 'cert-manager', ['webhook'], 3, 'Deployment/cert-manager-webhook', 'ip-10-0-4-2', 'Burstable', 21 * DAY, 'Pending', '10.244.4.40'),
      pod('etcd-backup-28391840-vl7xd', 'kube-system', ['backup'], 0, 'Job/etcd-backup-28391840', 'ip-10-0-4-2', 'BestEffort', 3 * HOUR, 'Completed', '10.244.4.44'),
      pod('report-generator-5f8b6d9c4-t8wvh', 'default', ['report'], 0, 'Deployment/report-generator', 'ip-10-0-4-19', 'Burstable', 35 * MIN, 'ContainerCreating', '10.244.4.52'),
      pod('node-exporter-8kz2s', 'monitoring', ['node-exporter'], 0, 'DaemonSet/node-exporter', 'ip-10-0-5-11', 'Burstable', 30 * DAY, 'Running', '10.244.5.3'),
      pod('argocd-application-controller-0', 'argocd', ['controller'], 0, 'StatefulSet/argocd-application-controller', 'ip-10-0-5-8', 'Guaranteed', 18 * DAY, 'Running', '10.244.5.9')
    ],
    deployments: [
      {name:'checkout-api', ns:'default', want:3, have:3, age:2 * DAY},
      {name:'payments-worker', ns:'default', want:2, have:0, age:40 * DAY},
      {name:'report-generator', ns:'default', want:1, have:0, age:35 * MIN},
      {name:'coredns', ns:'kube-system', want:2, have:2, age:120 * DAY},
      {name:'grafana', ns:'monitoring', want:1, have:1, age:90 * DAY},
      {name:'argocd-repo-server', ns:'argocd', want:1, have:1, age:60 * DAY},
      {name:'cert-manager-webhook', ns:'cert-manager', want:1, have:0, age:21 * DAY}
    ],
    daemonsets: [
      {name:'nginx-ingress', ns:'default', want:8, have:8, age:70 * DAY},
      {name:'node-exporter', ns:'monitoring', want:8, have:8, age:90 * DAY},
      {name:'kube-proxy', ns:'kube-system', want:8, have:7, age:120 * DAY}
    ],
    statefulsets: [
      {name:'prometheus-server', ns:'monitoring', want:1, have:1, age:90 * DAY},
      {name:'argocd-application-controller', ns:'argocd', want:1, have:1, age:60 * DAY},
      {name:'postgres-primary', ns:'default', want:3, have:3, age:200 * DAY}
    ],
    jobs: [
      {name:'etcd-backup-28391840', ns:'kube-system', want:1, have:1, age:3 * HOUR, state:'Complete'},
      {name:'db-migrate-v42', ns:'default', want:1, have:0, age:4 * MIN, state:'Running'},
      {name:'image-prune-28391200', ns:'default', want:1, have:1, age:14 * HOUR, state:'Complete'},
      {name:'cache-warm-28391700', ns:'default', want:1, have:0, age:2 * HOUR, state:'Failed'}
    ],
    cronjobs: [
      {name:'etcd-backup', ns:'kube-system', schedule:'0 */6 * * *', suspend:false, last:3 * HOUR, age:150 * DAY},
      {name:'image-prune', ns:'default', schedule:'0 3 * * *', suspend:false, last:14 * HOUR, age:150 * DAY},
      {name:'cache-warm', ns:'default', schedule:'*/30 * * * *', suspend:true, last:2 * HOUR, age:30 * DAY}
    ]
  };
  const ID_PREFIX = {deployments:'deployment', daemonsets:'daemonset', statefulsets:'statefulset', jobs:'job', cronjobs:'cronjob'};
  const OWNER_LABEL = {deployments:'Deployment', daemonsets:'DaemonSet', statefulsets:'StatefulSet', jobs:'Job'};
  Object.keys(ID_PREFIX).forEach(k => DATA[k].forEach(r => { r.id = ID_PREFIX[k] + '/' + r.ns + '/' + r.name; }));

  const podTone = s => s === 'Running' || s === 'Completed' ? 'success' : s === 'CrashLoopBackOff' || s === 'Failed' ? 'danger' : 'warn';
  const toneDot = t => t === 'success' ? 'ok' : t === 'danger' ? 'err' : 'warn';
  const pill = (label, tone) => '<span class="pill ' + tone + '"><span class="dot ' + toneDot(tone) + '"></span>' + esc(label) + '</span>';
  const nameCell = (icon, r) => '<span class="resource-name">' + ICON(icon) + esc(r.name) + '</span>';
  const nsCell = r => '<span class="namespace">' + nsLink(r.ns) + '</span>';
  const ageCell = r => '<span class="qos">' + fmtAge(r.age) + '</span>';
  const mono = (v, cls) => '<span class="' + (cls || 'qos') + '">' + esc(v) + '</span>';

  function podsOf(kindKey, r){
    const label = OWNER_LABEL[kindKey];
    return label ? DATA.pods.filter(p => p.ns === r.ns && p.ctrl === label + '/' + r.name) : [];
  }
  function ctrDots(p){
    const tone = p.status === 'Running' || p.status === 'Completed' ? 'ok' : p.status === 'CrashLoopBackOff' ? 'err' : 'warn';
    return '<span class="health-dots">' + p.ctrs.map(() => '<span class="dot ' + tone + '"></span>').join('') + '</span>';
  }
  const workloadState = r => r.have >= r.want ? ['Available', 'success'] : r.have === 0 ? ['Unavailable', 'danger'] : ['Progressing', 'warn'];
  const readyCols = icon => [
    {id:'name', label:'Name', val:r => r.name, html:r => nameCell(icon, r)},
    {id:'ns', label:'Namespace', val:r => r.ns, html:nsCell},
    {id:'ready', label:'Ready', val:r => r.have / r.want, html:r => mono(r.have + '/' + r.want, 'restarts')},
    {id:'age', label:'Age', val:r => r.age, html:ageCell},
    {id:'status', label:'Status', val:r => workloadState(r)[0], html:r => { const s = workloadState(r); return pill(s[0], s[1]); }}
  ];

  const KINDS = {
    pods: {
      label:'Pods', one:'Pod', sub:'Live pod inventory across the cluster. Select a row for details, or open logs and a shell from the actions menu.',
      cols:[
        {id:'name', label:'Name', val:r => r.name, html:r => nameCell('pods', r)},
        {id:'ns', label:'Namespace', val:r => r.ns, html:nsCell},
        {id:'health', label:'Health', val:r => r.status, html:ctrDots},
        {id:'restarts', label:'Restarts', val:r => r.restarts, html:r => mono(r.restarts, 'restarts')},
        {id:'ctrl', label:'Controller', val:r => r.ctrl, html:r => mono(r.ctrl, 'controller')},
        {id:'node', label:'Node', val:r => r.node, html:r => mono(r.node, 'node')},
        {id:'qos', label:'QoS', val:r => r.qos, html:r => mono(r.qos)},
        {id:'age', label:'Age', val:r => r.age, html:ageCell},
        {id:'status', label:'Status', val:r => r.status, html:r => pill(r.status, podTone(r.status))}
      ],
      search:r => [r.name, r.ns, r.node, r.ctrl, r.qos, r.status, r.ip].join(' '),
      actions:['logs', 'shell', 'edit', 'delete']
    },
    deployments: {
      label:'Deployments', one:'Deployment', sub:'Desired versus available replicas for each deployment. Scale or restart a rollout from the actions menu.',
      cols:readyCols('deploy'), search:r => [r.name, r.ns].join(' '), actions:['logs', 'scale', 'restart', 'edit', 'delete']
    },
    daemonsets: {
      label:'DaemonSets', one:'DaemonSet', sub:'One pod per matching node. Restart a rollout from the actions menu.',
      cols:readyCols('node'), search:r => [r.name, r.ns].join(' '), actions:['logs', 'restart', 'edit', 'delete']
    },
    statefulsets: {
      label:'StatefulSets', one:'StatefulSet', sub:'Ordered, stable pods with persistent identity. Scale or restart from the actions menu.',
      cols:readyCols('storage'), search:r => [r.name, r.ns].join(' '), actions:['logs', 'scale', 'restart', 'edit', 'delete']
    },
    jobs: {
      label:'Jobs', one:'Job', sub:'Run-to-completion workloads and their results.',
      cols:[
        {id:'name', label:'Name', val:r => r.name, html:r => nameCell('check', r)},
        {id:'ns', label:'Namespace', val:r => r.ns, html:nsCell},
        {id:'ready', label:'Completions', val:r => r.have / r.want, html:r => mono(r.have + '/' + r.want, 'restarts')},
        {id:'age', label:'Age', val:r => r.age, html:ageCell},
        {id:'status', label:'Status', val:r => r.state, html:r => pill(r.state, r.state === 'Complete' ? 'success' : r.state === 'Failed' ? 'danger' : 'warn')}
      ],
      search:r => [r.name, r.ns, r.state].join(' '), actions:['logs', 'edit', 'delete']
    },
    cronjobs: {
      label:'CronJobs', one:'CronJob', sub:'Scheduled jobs. Trigger a run now without waiting for the schedule.',
      cols:[
        {id:'name', label:'Name', val:r => r.name, html:r => nameCell('clock', r)},
        {id:'ns', label:'Namespace', val:r => r.ns, html:nsCell},
        {id:'schedule', label:'Schedule', val:r => r.schedule, html:r => mono(r.schedule, 'controller')},
        {id:'last', label:'Last run', val:r => r.last, html:r => mono(fmtAge(r.last) + ' ago')},
        {id:'age', label:'Age', val:r => r.age, html:ageCell},
        {id:'status', label:'Status', val:r => r.suspend ? 'Suspended' : 'Active', html:r => r.suspend ? pill('Suspended', 'warn') : pill('Active', 'success')}
      ],
      search:r => [r.name, r.ns, r.schedule].join(' '), actions:['trigger', 'edit', 'delete']
    }
  };

  const ACTION = {
    logs:{label:'Logs', icon:'logs'}, shell:{label:'Shell', icon:'terminal'}, scale:{label:'Scale', icon:'scale'},
    restart:{label:'Restart', icon:'refresh'}, trigger:{label:'Trigger now', icon:'sync'}, edit:{label:'Edit', icon:'pencil'},
    delete:{label:'Delete', icon:'trash', danger:true}
  };

  /* ---------------------------------------------- generic resource catalogue */
  const T = (keys, rows) => rows.map(a => Object.fromEntries(keys.map((k, i) => [k, a[i]])));
  const NSN = ['default', 'kube-system', 'monitoring', 'argocd', 'cert-manager'];
  const G = {
    nodes:{group:'Cluster', label:'Nodes', one:'Node', icon:'node', cluster:true, api:'v1', actions:['cordon', 'drain', 'edit'], sub:'Machines in the cluster with their readiness and utilisation.',
      cols:[['name','Name'], ['status','Status','pill'], ['roles','Roles'], ['version','Version'], ['cpu','CPU'], ['mem','Memory'], ['age','Age','age']],
      rows:T(['name','status','roles','version','cpu','mem','age'], [
        ['ip-10-0-4-2','Ready','control-plane','v1.29.4','31%','48%',120 * DAY], ['ip-10-0-4-12','Ready','worker','v1.29.4','44%','52%',120 * DAY],
        ['ip-10-0-4-18','Ready','worker','v1.29.4','58%','61%',98 * DAY], ['ip-10-0-4-19','Ready','worker','v1.29.4','72%','66%',98 * DAY],
        ['ip-10-0-5-3','Ready,SchedulingDisabled','worker','v1.29.4','12%','28%',60 * DAY], ['ip-10-0-5-8','Ready','worker','v1.29.4','39%','44%',60 * DAY],
        ['ip-10-0-5-11','Ready','worker','v1.29.4','47%','57%',60 * DAY], ['ip-10-0-5-14','Ready','worker','v1.29.4','25%','35%',30 * DAY]])},
    namespaces:{group:'Cluster', label:'Namespaces', one:'Namespace', icon:'cluster', cluster:true, api:'v1', sub:'Logical partitions of the cluster.',
      cols:[['name','Name'], ['status','Status','pill'], ['owner','Managed by'], ['age','Age','age']],
      rows:T(['name','status','owner','age'], [['default','Active','—',150 * DAY], ['kube-system','Active','kubeadm',150 * DAY], ['kube-public','Active','kubeadm',150 * DAY], ['monitoring','Active','helm',90 * DAY], ['argocd','Active','argocd',60 * DAY], ['cert-manager','Active','helm',90 * DAY]])},
    events:{group:'Cluster', label:'Events', one:'Event', icon:'warn', api:'v1', actions:[], sub:'Recent cluster events, newest first.',
      cols:[['name','Object'], ['type','Type','pill'], ['reason','Reason'], ['message','Message'], ['age','Age','age']],
      rows:T(['ns','name','type','reason','message','age'], [
        ['default','pod/payments-worker-6b8d9c7f5d-9nq4r','Warning','BackOff','Back-off restarting failed container',6 * MIN], ['cert-manager','pod/cert-manager-webhook-6d5c9b7f4-qk2lp','Warning','FailedMount','MountVolume.SetUp failed for volume webhook-tls',34 * MIN],
        ['default','deployment/report-generator','Normal','ScalingReplicaSet','Scaled up replica set report-generator-5f8b6d9c4 to 1',35 * MIN], ['argocd','application/canary-rollouts','Warning','OutOfSync','Drift detected against main',8 * MIN],
        ['monitoring','statefulset/prometheus-server','Normal','SuccessfulCreate','create Pod prometheus-server-0 successful',2 * HOUR], ['kube-system','job/etcd-backup-28391840','Normal','Completed','Job completed',3 * HOUR],
        ['default','deployment/checkout-api','Normal','ScalingReplicaSet','Scaled up replica set checkout-api-7c9f8d6b4f to 3',2 * DAY], ['monitoring','pod/grafana-7f6d4c8b5d-mz8kt','Warning','Unhealthy','Readiness probe failed: HTTP 503',50 * MIN],
        ['argocd','pod/argocd-repo-server-55b7c44d9c-7b2wm','Warning','OOMKilled','Container repo-server exceeded memory limit',5 * HOUR], ['default','pod/nginx-ingress-controller-5d9f8c6b7-x4z1p','Normal','Pulled','Successfully pulled image',4 * DAY]])},
    crds:{group:'Custom Resources', label:'Definitions', one:'CustomResourceDefinition', icon:'projects', cluster:true, api:'apiextensions.k8s.io/v1', sub:'Custom resource definitions registered with the API server.',
      cols:[['name','Name'], ['group','Group'], ['scope','Scope'], ['versions','Versions'], ['instances','Instances','drill'], ['age','Age','age']],
      rows:T(['name','group','scope','versions','age'], [
        ['certificates.cert-manager.io','cert-manager.io','Namespaced','v1',90 * DAY], ['clusterissuers.cert-manager.io','cert-manager.io','Cluster','v1',90 * DAY],
        ['applications.argoproj.io','argoproj.io','Namespaced','v1alpha1',60 * DAY], ['appprojects.argoproj.io','argoproj.io','Namespaced','v1alpha1',60 * DAY],
        ['servicemonitors.monitoring.coreos.com','monitoring.coreos.com','Namespaced','v1',90 * DAY], ['prometheusrules.monitoring.coreos.com','monitoring.coreos.com','Namespaced','v1',90 * DAY]])},
    certificates:{group:'Custom Resources', label:'Certificates', one:'Certificate', icon:'check', api:'cert-manager.io/v1', sub:'cert-manager certificates and their readiness.',
      cols:[['name','Name'], ['ready','Ready','pill'], ['secret','Secret'], ['issuer','Issuer'], ['age','Age','age']],
      rows:T(['ns','name','ready','secret','issuer','age'], [['default','checkout-tls','True','checkout-tls','letsencrypt-prod',40 * DAY], ['argocd','argocd-server-tls','True','argocd-server-tls','letsencrypt-prod',60 * DAY], ['monitoring','grafana-tls','False','grafana-tls','letsencrypt-staging',2 * DAY]])},
    servicemonitors:{group:'Custom Resources', label:'ServiceMonitors', one:'ServiceMonitor', icon:'network', api:'monitoring.coreos.com/v1', sub:'Prometheus scrape targets declared as custom resources.',
      cols:[['name','Name'], ['selector','Selector'], ['interval','Interval'], ['age','Age','age']],
      rows:T(['ns','name','selector','interval','age'], [['monitoring','kubelet','k8s-app=kubelet','30s',90 * DAY], ['monitoring','node-exporter','app=node-exporter','30s',90 * DAY], ['default','checkout-api','app=checkout-api','15s',20 * DAY]])},
    replicasets:{group:'Workloads', label:'ReplicaSets', one:'ReplicaSet', icon:'deploy', api:'apps/v1', sub:'Replica sets owned by deployments.',
      cols:[['name','Name'], ['desired','Desired'], ['current','Current'], ['ready','Ready'], ['age','Age','age']],
      rows:T(['ns','name','desired','current','ready','age'], [['default','checkout-api-7c9f8d6b4f',3,3,3,2 * DAY], ['default','payments-worker-6b8d9c7f5d',2,2,0,6 * HOUR], ['monitoring','grafana-7f6d4c8b5d',1,1,1,11 * DAY], ['kube-system','coredns-5c98db65d4',2,2,2,12 * DAY]])},
    configmaps:{group:'Config', label:'ConfigMaps', one:'ConfigMap', icon:'config', api:'v1', sub:'Non-sensitive configuration data.',
      cols:[['name','Name'], ['keys','Keys'], ['age','Age','age']],
      rows:T(['ns','name','keys','age'], [['default','checkout-config','app.yaml, feature-flags.json',20 * DAY], ['kube-system','coredns','Corefile',150 * DAY], ['monitoring','prometheus-rules','alerts.yaml, recording.yaml',60 * DAY], ['argocd','argocd-cm','url, repositories',60 * DAY]])},
    secrets:{group:'Config', label:'Secrets', one:'Secret', icon:'config', api:'v1', sub:'Sensitive values. Contents are never shown in the list.',
      cols:[['name','Name'], ['type','Type'], ['keys','Keys'], ['age','Age','age']],
      rows:T(['ns','name','type','keys','age'], [['default','checkout-tls','kubernetes.io/tls',2,40 * DAY], ['default','payments-db','Opaque',3,40 * DAY], ['argocd','argocd-secret','Opaque',4,60 * DAY], ['monitoring','grafana-admin','Opaque',2,90 * DAY]])},
    hpas:{group:'Config', label:'HPA', one:'HorizontalPodAutoscaler', icon:'scale', api:'autoscaling/v2', sub:'Horizontal pod autoscalers and their targets.',
      cols:[['name','Name'], ['target','Target'], ['min','Min'], ['max','Max'], ['replicas','Replicas'], ['age','Age','age']],
      rows:T(['ns','name','target','min','max','replicas','age'], [['default','checkout-api','Deployment/checkout-api',2,8,3,20 * DAY], ['default','payments-worker','Deployment/payments-worker',2,6,2,40 * DAY]])},
    services:{group:'Network', label:'Services', one:'Service', icon:'services', api:'v1', actions:['forward', 'edit', 'delete'], sub:'Stable network endpoints for sets of pods.',
      cols:[['name','Name'], ['type','Type'], ['clusterIP','Cluster IP'], ['ports','Ports'], ['age','Age','age']],
      rows:T(['ns','name','type','clusterIP','ports','age'], [['default','checkout-api','ClusterIP','10.96.12.40','80/TCP',20 * DAY], ['default','nginx-ingress','LoadBalancer','10.96.4.8','80:30080/TCP, 443:30443/TCP',70 * DAY], ['kube-system','kube-dns','ClusterIP','10.96.0.10','53/UDP, 53/TCP',150 * DAY], ['monitoring','prometheus','ClusterIP','10.96.31.2','9090/TCP',90 * DAY], ['argocd','argocd-server','ClusterIP','10.96.77.9','80/TCP, 443/TCP',60 * DAY]])},
    ingresses:{group:'Network', label:'Ingresses', one:'Ingress', icon:'network', api:'networking.k8s.io/v1', sub:'HTTP routes into the cluster.',
      cols:[['name','Name'], ['class','Class'], ['hosts','Hosts'], ['address','Address'], ['age','Age','age']],
      rows:T(['ns','name','class','hosts','address','age'], [['default','checkout','nginx','shop.example.com','34.120.8.14',20 * DAY], ['argocd','argocd-server','nginx','argocd.example.com','34.120.8.14',60 * DAY], ['monitoring','grafana','nginx','grafana.example.com','34.120.8.14',90 * DAY]])},
    networkpolicies:{group:'Network', label:'NetworkPolicies', one:'NetworkPolicy', icon:'network', api:'networking.k8s.io/v1', sub:'Pod-level traffic rules.',
      cols:[['name','Name'], ['podSelector','Pod selector'], ['policyTypes','Types'], ['age','Age','age']],
      rows:T(['ns','name','podSelector','policyTypes','age'], [['default','deny-all','<all pods>','Ingress',100 * DAY], ['default','allow-checkout','app=checkout-api','Ingress, Egress',20 * DAY], ['monitoring','allow-scrape','app=prometheus','Ingress',90 * DAY]])},
    pvcs:{group:'Storage', label:'PersistentVolumeClaims', one:'PersistentVolumeClaim', icon:'storage', api:'v1', sub:'Storage requested by workloads.',
      cols:[['name','Name'], ['status','Status','pill'], ['volume','Volume'], ['capacity','Capacity'], ['storageClass','Storage class'], ['age','Age','age']],
      rows:T(['ns','name','status','volume','capacity','storageClass','age'], [['default','postgres-data-0','Bound','pvc-8a1f…','50Gi','gp3',200 * DAY], ['monitoring','prometheus-data','Bound','pvc-2c9e…','100Gi','gp3',90 * DAY], ['monitoring','grafana-data','Pending','—','10Gi','gp3',2 * HOUR]])},
    pvs:{group:'Storage', label:'PersistentVolumes', one:'PersistentVolume', icon:'storage', cluster:true, api:'v1', sub:'Cluster-wide volumes backing claims.',
      cols:[['name','Name'], ['status','Status','pill'], ['claim','Claim'], ['capacity','Capacity'], ['storageClass','Storage class'], ['age','Age','age']],
      rows:T(['name','status','claim','capacity','storageClass','age'], [['pvc-8a1f4d','Bound','default/postgres-data-0','50Gi','gp3',200 * DAY], ['pvc-2c9e71','Bound','monitoring/prometheus-data','100Gi','gp3',90 * DAY], ['pvc-77b3e0','Released','default/old-cache','20Gi','gp2',300 * DAY]])},
    storageclasses:{group:'Storage', label:'StorageClasses', one:'StorageClass', icon:'storage', cluster:true, api:'storage.k8s.io/v1', sub:'Provisioners and policies for dynamic volumes.',
      cols:[['name','Name'], ['provisioner','Provisioner'], ['reclaim','Reclaim policy'], ['default','Default'], ['age','Age','age']],
      rows:T(['name','provisioner','reclaim','default','age'], [['gp3','ebs.csi.aws.com','Delete','Yes',150 * DAY], ['gp2','kubernetes.io/aws-ebs','Delete','No',300 * DAY], ['efs-sc','efs.csi.aws.com','Retain','No',100 * DAY]])},
    serviceaccounts:{group:'Access Control', label:'ServiceAccounts', one:'ServiceAccount', icon:'projects', api:'v1', sub:'Identities used by pods.',
      cols:[['name','Name'], ['secrets','Secrets'], ['age','Age','age']],
      rows:T(['ns','name','secrets','age'], [['default','default',0,150 * DAY], ['argocd','argocd-application-controller',1,60 * DAY], ['monitoring','prometheus',1,90 * DAY], ['cert-manager','cert-manager',0,90 * DAY]])},
    roles:{group:'Access Control', label:'Roles', one:'Role', icon:'projects', api:'rbac.authorization.k8s.io/v1', sub:'Namespaced permission sets.',
      cols:[['name','Name'], ['rules','Rules'], ['age','Age','age']],
      rows:T(['ns','name','rules','age'], [['argocd','argocd-application-controller',3,60 * DAY], ['monitoring','prometheus',4,90 * DAY], ['kube-system','extension-apiserver-authentication-reader',1,150 * DAY]])},
    clusterroles:{group:'Access Control', label:'ClusterRoles', one:'ClusterRole', icon:'projects', cluster:true, api:'rbac.authorization.k8s.io/v1', sub:'Cluster-wide permission sets.',
      cols:[['name','Name'], ['rules','Rules'], ['age','Age','age']],
      rows:T(['name','rules','age'], [['cluster-admin',1,150 * DAY], ['view',12,150 * DAY], ['edit',34,150 * DAY], ['system:node',9,150 * DAY]])},
    rolebindings:{group:'Access Control', label:'RoleBindings', one:'RoleBinding', icon:'projects', api:'rbac.authorization.k8s.io/v1', sub:'Who is granted which role.',
      cols:[['name','Name'], ['role','Role'], ['subjects','Subjects'], ['age','Age','age']],
      rows:T(['ns','name','role','subjects','age'], [['argocd','argocd-application-controller','Role/argocd-application-controller','ServiceAccount/argocd-application-controller',60 * DAY], ['monitoring','prometheus','Role/prometheus','ServiceAccount/prometheus',90 * DAY]])},
    argoapps:{group:'Custom Resources', hidden:true, label:'Applications', one:'Application', icon:'argo', api:'argoproj.io/v1alpha1', sub:'Argo CD applications (argoproj.io).',
      cols:[['name','Name'], ['sync','Sync','pill'], ['health','Health','pill'], ['repo','Repository'], ['age','Age','age']],
      rows:T(['ns','name','sync','health','repo','age'], [['argocd','guestbook','Synced','Healthy','github.com/acme/guestbook',60 * DAY], ['argocd','canary-rollouts','OutOfSync','Progressing','github.com/acme/rollouts',20 * DAY], ['argocd','payments','Synced','Degraded','github.com/acme/payments',40 * DAY]])},
    appprojects:{group:'Custom Resources', hidden:true, label:'AppProjects', one:'AppProject', icon:'projects', api:'argoproj.io/v1alpha1', sub:'Argo CD projects.',
      cols:[['name','Name'], ['description','Description'], ['destinations','Destinations'], ['age','Age','age']],
      rows:T(['ns','name','description','destinations','age'], [['argocd','default','Default project','*',60 * DAY], ['argocd','platform','Platform services','3',50 * DAY]])},
    clusterissuers:{group:'Custom Resources', hidden:true, label:'ClusterIssuers', one:'ClusterIssuer', icon:'check', cluster:true, api:'cert-manager.io/v1', sub:'cert-manager cluster issuers.',
      cols:[['name','Name'], ['ready','Ready','pill'], ['type','Type'], ['age','Age','age']],
      rows:T(['name','ready','type','age'], [['letsencrypt-prod','True','ACME',90 * DAY], ['letsencrypt-staging','True','ACME',90 * DAY]])},
    prometheusrules:{group:'Custom Resources', hidden:true, label:'PrometheusRules', one:'PrometheusRule', icon:'warn', api:'monitoring.coreos.com/v1', sub:'Alerting and recording rules.',
      cols:[['name','Name'], ['groups','Groups'], ['rules','Rules'], ['age','Age','age']],
      rows:T(['ns','name','groups','rules','age'], [['monitoring','kubernetes-apps',3,24,90 * DAY], ['monitoring','node-exporter',2,11,90 * DAY]])},
    helm:{group:'Helm', label:'Releases', one:'Release', icon:'repo', api:'helm.sh/v3', actions:['resources', 'rollback', 'delete'], sub:'Helm releases installed in the cluster. Expand a row, or open Resources, to see everything a chart deployed.',
      cols:[['name','Name'], ['chart','Chart'], ['revision','Revision'], ['status','Status','pill'], ['resources','Resources','hres'], ['age','Updated','age']],
      rows:T(['ns','name','chart','revision','status','age'], [['monitoring','kube-prometheus-stack','kube-prometheus-stack-58.2.1',7,'Deployed',9 * DAY], ['cert-manager','cert-manager','cert-manager-v1.14.4',3,'Deployed',90 * DAY], ['default','checkout','checkout-0.18.2',12,'Deployed',2 * DAY], ['default','report-generator','report-generator-0.2.0',1,'Failed',35 * MIN]])}
  };
  const tone = v => /Disabled|Pending|Warning|Released|Progressing|Unknown|Superseded/.test(String(v)) ? 'warn' : /Fail|NotReady|Lost|Error|Degraded|False/.test(String(v)) ? 'danger' : 'success';
  Object.keys(G).forEach(k => {
    const g = G[k], cols = [];
    DATA[k] = g.rows;
    DATA[k].forEach(r => { r.id = k + '/' + (r.ns || '-') + '/' + r.name; });
    g.cols.forEach((c, i) => {
      const f = c[0], label = c[1], type = c[2];
      cols.push({id:f, label,
        val:r => r[f],
        html:r => f === 'name' ? nameCell(g.icon, r) : type === 'pill' ? pill(String(r[f]), tone(r[f])) : type === 'hres' ? resOrb(r, r.id) : type === 'drill' ? '<button type="button" class="link-btn" data-drill="' + esc(r.name) + '">' + cnt(r[f]) + ' →</button>' : type === 'age' ? ageCell(r) : mono(r[f] == null ? '—' : r[f], 'qos')});
      if (i === 0 && !g.cluster) cols.push({id:'ns', label:'Namespace', val:r => r.ns, html:nsCell});
    });
    KINDS[k] = {group:g.group, label:g.label, one:g.one, icon:g.icon, cluster:!!g.cluster, generic:true, api:g.api, sub:g.sub, cols,
      search:r => Object.values(r).join(' '), actions:g.actions || ['edit', 'delete']};
  });
  ['pods', 'deployments', 'daemonsets', 'statefulsets', 'jobs', 'cronjobs'].forEach(k => { KINDS[k].group = 'Workloads'; });
  KINDS.pods.icon = 'pods'; KINDS.deployments.icon = 'deploy'; KINDS.daemonsets.icon = 'node'; KINDS.statefulsets.icon = 'storage'; KINDS.jobs.icon = 'check'; KINDS.cronjobs.icon = 'clock';
  KINDS.pods.actions = ['logs', 'shell', 'forward', 'edit', 'delete'];
  Object.assign(ACTION, {
    resources:{label:'Resources', icon:'deploy'}, forward:{label:'Port forward', icon:'plug'}, cordon:{label:'Cordon', icon:'warn'}, drain:{label:'Drain', icon:'refresh'},
    rollback:{label:'Roll back', icon:'sync'}, copy:{label:'Copy name', icon:'copy'}
  });

  const CRD_KIND = {'certificates.cert-manager.io':'certificates', 'clusterissuers.cert-manager.io':'clusterissuers', 'applications.argoproj.io':'argoapps', 'appprojects.argoproj.io':'appprojects', 'servicemonitors.monitoring.coreos.com':'servicemonitors', 'prometheusrules.monitoring.coreos.com':'prometheusrules'};
  DATA.crds.forEach(x => { x.instances = (DATA[CRD_KIND[x.name]] || []).length; });
  /* what each Helm release deployed: [kind, resource name] pairs, resolved against the live data */
  const HELM_MAP = {
    'monitoring/kube-prometheus-stack':[['statefulsets', 'prometheus-server'], ['deployments', 'grafana'], ['daemonsets', 'node-exporter'], ['services', 'prometheus'], ['ingresses', 'grafana'], ['configmaps', 'prometheus-rules'], ['secrets', 'grafana-admin'], ['serviceaccounts', 'prometheus'], ['roles', 'prometheus'], ['rolebindings', 'prometheus'], ['pvcs', 'prometheus-data'], ['pvcs', 'grafana-data'], ['servicemonitors', 'kubelet'], ['servicemonitors', 'node-exporter'], ['networkpolicies', 'allow-scrape'], ['certificates', 'grafana-tls']],
    'cert-manager/cert-manager':[['deployments', 'cert-manager-webhook'], ['serviceaccounts', 'cert-manager'], ['crds', 'certificates.cert-manager.io'], ['crds', 'clusterissuers.cert-manager.io'], ['clusterissuers', 'letsencrypt-prod'], ['clusterissuers', 'letsencrypt-staging']],
    'default/checkout':[['deployments', 'checkout-api'], ['services', 'checkout-api'], ['ingresses', 'checkout'], ['configmaps', 'checkout-config'], ['secrets', 'checkout-tls'], ['hpas', 'checkout-api'], ['certificates', 'checkout-tls'], ['networkpolicies', 'allow-checkout']],
    'default/report-generator':[['deployments', 'report-generator']]
  };
  function helmResources(rel){
    return (HELM_MAP[rel.ns + '/' + rel.name] || []).map(p => { const row = DATA[p[0]] && DATA[p[0]].find(x => x.name === p[1] && (!x.ns || x.ns === rel.ns)); return row ? {kind:p[0], r:row} : null; }).filter(Boolean);
  }
  DATA.helm.forEach(x => { x.resources = helmResources(x).length; });
  function helmHealth(rel){
    const items = helmResources(rel);
    let ok = 0, warn = 0, bad = 0;
    items.forEach(x => { const st = statusOf(x.kind, x.r), t = st ? st[1] : 'success'; if (t === 'danger') bad++; else if (t === 'warn') warn++; else ok++; });
    return {n:items.length, ok, warn, bad};
  }
  const cnt = (n, cls) => '<span class="cnt' + (cls ? ' ' + cls : '') + '">' + n + '</span>';
  function resOrb(rel, id){
    const h = helmHealth(rel), n = Math.max(1, h.n), a = h.ok / n * 100, b = (h.ok + h.warn) / n * 100;
    const state = h.bad ? h.bad + ' failing' : h.warn ? h.warn + ' progressing' : 'all healthy';
    return '<button type="button" class="orb-count' + (h.bad ? ' bad' : h.warn ? ' warn' : '') + '" ' + (id ? 'data-hres="' + esc(id) + '" ' : '') + 'style="--a:' + a.toFixed(1) + '%;--b:' + b.toFixed(1) + '%" aria-label="' + h.n + ' resources, ' + state + '. Open resources" title="' + h.n + ' resources · ' + state + '"><span>' + h.n + '</span></button>';
  }
  /* sparkline columns: CPU shows by default where it did before; memory, network and disk are optional (hidden until chosen in the column picker) */
  const TREND_DEFS = [
    {id:'trend', label:'CPU trend', color:'var(--accent)', pick:m => m.cpu.data, fmt:F => F.cpu},
    {id:'trend-mem', label:'Memory trend', color:'var(--violet)', pick:m => m.mem.data, fmt:F => F.mem},
    {id:'trend-net', label:'Network trend', color:'var(--success)', pick:m => OKCharts.sumSeries([m.rx, m.tx]), fmt:F => F.bps},
    {id:'trend-fs', label:'Disk I/O trend', color:'var(--brand)', pick:m => OKCharts.sumSeries([m.fr, m.fw]), fmt:F => F.bps}
  ];
  ['pods', 'deployments', 'statefulsets', 'daemonsets', 'replicasets', 'namespaces', 'nodes'].forEach(k => {
    const cols = KINDS[k].cols, shownByDefault = ['deployments', 'statefulsets', 'daemonsets', 'namespaces', 'nodes'].indexOf(k) >= 0;
    let at = cols.findIndex(x => x.id === 'age'); if (at < 0) at = cols.length;
    KINDS[k].defaultHide = [];
    TREND_DEFS.forEach((t, i) => {
      cols.splice(at + i, 0, {id:t.id, label:t.label,
        val:x => OKCharts.last(t.pick(metricsOf(k, x, '1h'))),
        html:x => { const d = t.pick(metricsOf(k, x, '1h')); return '<span class="trend-cell">' + OKCharts.spark(d, t.color) + t.fmt(OKCharts.FMT)(OKCharts.last(d)) + '</span>'; }});
      if (!(i === 0 && shownByDefault)) KINDS[k].defaultHide.push(t.id);
    });
  });
  KINDS.events.defaultSort = {id:'age', dir:1};
  const NAV = [
    {g:'Cluster', items:[['view', 'overview', 'Overview', 'cluster'], ['kind', 'nodes'], ['kind', 'namespaces'], ['kind', 'events']]},
    {g:'Workloads', items:[['kind', 'pods'], ['kind', 'deployments'], ['kind', 'daemonsets'], ['kind', 'statefulsets'], ['kind', 'replicasets'], ['kind', 'jobs'], ['kind', 'cronjobs']]},
    {g:'Config', items:[['kind', 'configmaps'], ['kind', 'secrets'], ['kind', 'hpas']]},
    {g:'Network', items:[['kind', 'services'], ['kind', 'ingresses'], ['kind', 'networkpolicies']]},
    {g:'Storage', items:[['kind', 'pvcs'], ['kind', 'pvs'], ['kind', 'storageclasses']]},
    {g:'Access Control', items:[['kind', 'serviceaccounts'], ['kind', 'roles'], ['kind', 'clusterroles'], ['kind', 'rolebindings']]},
    {g:'Helm', items:[['kind', 'helm']]},
    {g:'Custom Resources', items:[['kind', 'crds'], ['kind', 'certificates'], ['kind', 'servicemonitors']]},
    {g:'Argo CD', argo:true, items:[['view', 'argocd', 'Applications', 'argo'], ['soon', 'Projects', 'projects'], ['soon', 'Repositories', 'repo']]}
  ];
  // Cluster and Workloads stay open; every other category auto-collapses (only the one you're in or just opened stays expanded)
  const NAV_PINNED = ['Cluster', 'Workloads'];
  const navOpen = new Set(NAV_PINNED);

  /* --------------------------------------------------------------- state */
  const S = {kind:'pods', ns:new Set(), q:'', sort:{id:null, dir:1}, sel:new Set(), inspect:null, inspTab:'overview', view:'workloads'};
  /* ---- back stack: every "step through" (a link that jumps to another resource, app or page) remembers where you were,
     and one Back control, wherever you are, returns there. Starting somewhere fresh (menu, row click, search, closing the pane) clears it. */
  const NAV_STACK = [];
  let navKeep = false;
  function navSnap(){
    const paneOpen = inspector.classList.contains('open');
    let pane = null, label;
    if (paneOpen && S.argoView) { pane = {t:'argo', name:S.argoView.name, kind:S.argoView.kind, tab:S.argoView.tab}; label = S.argoView.name; }
    else if (paneOpen && S.metricView) { pane = {t:'metric', metric:S.metricView.metric, scope:S.metricView.scope}; label = (MD[S.metricView.metric] || {title:'Metric'}).title + ' metrics'; }
    else if (paneOpen && S.inspect && byId(S.inspect)) { pane = {t:'inspect', id:S.inspect, tab:S.inspTab}; label = byId(S.inspect).r.name; }
    else label = S.view === 'overview' ? 'Overview' : S.view === 'argocd' ? 'Argo CD' : KINDS[S.kind].label;
    return {view:S.view, kind:S.kind, atab:AS.tab, pane, label, scroll:main.scrollTop};
  }
  function navUI(){
    const top = NAV_STACK[NAV_STACK.length - 1], on = !!top;
    const b = $('#navBack'), ib = $('#inspBack');
    b.hidden = !on; ib.hidden = !on || !inspector.classList.contains('open');
    if (on) { const t = 'Back to ' + top.label; b.setAttribute('aria-label', t); b.title = t + '  (Alt+←)'; ib.setAttribute('aria-label', t); ib.title = t; }
    const wb = $('#wlBack'); if (wb) { wb.hidden = !on && !S.from; if (on) wb.textContent = '← ' + top.label; else if (S.from) wb.textContent = '← Definitions'; }
  }
  function navPush(){
    const s = navSnap(), top = NAV_STACK[NAV_STACK.length - 1];
    if (top && JSON.stringify(Object.assign({}, top, {scroll:0})) === JSON.stringify(Object.assign({}, s, {scroll:0}))) return;
    NAV_STACK.push(s); if (NAV_STACK.length > 25) NAV_STACK.shift();
    navUI();
  }
  const navClear = () => { if (navKeep || !NAV_STACK.length) return; NAV_STACK.length = 0; navUI(); };
  function navBack(){
    const s = NAV_STACK.pop();
    if (!s) return;
    navKeep = true;
    try {
      if (s.view !== S.view || (s.view === 'workloads' && s.kind !== S.kind)) setView(s.view, s.kind);
      if (s.view === 'argocd') { AS.tab = s.atab; renderArgo(); }
      const p = s.pane;
      if (!p) closeInspector();
      else if (p.t === 'inspect') { openInspector(p.id); S.inspTab = p.tab; renderInspector(p.id); }
      else if (p.t === 'argo') openArgoDetail(p.name, p.tab, p.kind);
      else openMetricDetail(p.metric, p.scope);
      main.scrollTop = s.scroll || 0;
    } finally { navKeep = false; }
    navUI();
  }
  const stepTo = fn => { navPush(); navKeep = true; try { fn(); } finally { navKeep = false; } navUI(); };
  const rowsOf = () => DATA[S.kind];
  function byId(id){
    for (const k of Object.keys(DATA)) { const r = DATA[k].find(x => x.id === id); if (r) return {r, kind:k}; }
    return null;
  }

  /* ---------------------------------------------------------- workloads UI */
  const head = $('#wlHead'), tbody = $('#podRows'), chipsEl = $('#nsChips'), search = $('#podSearch');
  const bulkBar = $('#bulkBar');
  function updateFlOrb(rows){
    const q = S.q.trim(), total = rowsOf().length;
    $('#flBadge').innerHTML = q ? cnt(rows, 'on') + ' of ' + cnt(total) : '<span class="all-badge">All ' + cnt(total) + '</span>';
    $('#flClear').hidden = !q;
    $('#podSearch').closest('label').classList.toggle('filtered', !!q);
  }

  /* columns: per-resource-type hidden list; sort: default is name A-Z, a user-chosen sort is locked in per type until reset */
  const lsRead = (k, d) => { try { return JSON.parse(localStorage.getItem(k) || ''); } catch (err) { return d; } };
  const lsWrite = (k, v) => { try { localStorage.setItem(k, JSON.stringify(v)); } catch (err) { /* private mode */ } };
  S.colHide = lsRead('openkite-cols', {}) || {};
  S.sortPref = lsRead('openkite-sort', {}) || {};
  const DEFAULT_SORT = {id:'name', dir:1};
  const defaultSort = kind => KINDS[kind].defaultSort || DEFAULT_SORT;   // events read newest first; everything else A-Z by name
  const curSort = () => { const p = S.sortPref[S.kind]; return p && KINDS[S.kind].cols.some(c => c.id === p.id) ? p : defaultSort(S.kind); };
  const hiddenOf = kind => S.colHide[kind] || KINDS[kind].defaultHide || [];
  S.colOrder = lsRead('openkite-colorder', {}) || {};
  // Name stays first; the rest follow the saved order, with any unknown/new column kept in its natural spot at the end
  const orderedCols = K => {
    const ord = S.colOrder[S.kind] || [], first = K.cols[0], rest = K.cols.slice(1);
    return [first].concat(rest.slice().sort((a, b) => { const x = ord.indexOf(a.id), y = ord.indexOf(b.id); return (x < 0 ? 1e3 + rest.indexOf(a) : x) - (y < 0 ? 1e3 + rest.indexOf(b) : y); }));
  };
  const shownCols = K => { const hide = hiddenOf(S.kind); return orderedCols(K).filter((c, i) => i === 0 || hide.indexOf(c.id) < 0); };
  function visibleRows(){
    const K = KINDS[S.kind], q = S.q.trim().toLowerCase();
    let rows = rowsOf().filter(r => inNs(r) && (!S.flt || S.flt.fn(r)) && (!q || K.search(r).toLowerCase().includes(q)));
    const so = curSort(), col = K.cols.find(c => c.id === so.id);
    if (col) rows = rows.slice().sort((a, b) => {
      const x = col.val(a), y = col.val(b);
      return (typeof x === 'number' ? x - y : String(x).localeCompare(String(y), undefined, {numeric:true, sensitivity:'base'})) * so.dir;
    });
    return rows;
  }

  function renderHead(){
    const K = KINDS[S.kind];
    head.innerHTML = '<th class="col-check"><input type="checkbox" id="selAll" aria-label="Select all"></th>' +
      shownCols(K).map(c => {
        const so = curSort(), on = so.id === c.id;
        return '<th aria-sort="' + (on ? (so.dir > 0 ? 'ascending' : 'descending') : 'none') + '"><button type="button" class="th-sort" data-sort="' + c.id + '">' + esc(c.label) + '<span class="sort-mark">' + (on ? (so.dir > 0 ? '↑' : '↓') : '') + '</span></button></th>';
      }).join('') + '<th class="col-act"><span class="sr-only">Actions</span></th>';
  }

  function renderChips(){
    const flt = S.flt && !S.flt.evtype ? '<button class="chip active flt" type="button" data-flt="clear" aria-label="Clear filter: ' + esc(S.flt.label) + '">' + esc(S.flt.label) + '<span class="chip-x" aria-hidden="true">×</span></button>' : '';
    const cluster = KINDS[S.kind].cluster, isEv = S.kind === 'events';
    let evChips = '';
    if (isEv) {
      const evs = NSD('events'), cur = S.flt && S.flt.evtype || 'all';
      evChips = '<span class="seg" role="group" aria-label="Event type">' + [['all', 'All', evs.length], ['Warning', 'Warnings', evs.filter(e => e.type === 'Warning').length], ['Normal', 'Normal', evs.filter(e => e.type === 'Normal').length]].map(x =>
        '<button class="chip' + (cur === x[0] ? ' active' : '') + '" type="button" data-evtype="' + x[0] + '" aria-pressed="' + (cur === x[0]) + '">' + x[1] + cnt(x[2]) + '</button>').join('') + '</span>';
    }
    chipsEl.hidden = !isEv && !flt;
    chipsEl.innerHTML = evChips + flt;
  }

  function renderRows(){
    const K = KINDS[S.kind], rows = visibleRows();
    childReg.clear();
    const cols = shownCols(K), mob = cols.some(c => c.id === 'status') ? 'status' : (cols.filter(c => c.id !== 'name' && c.id !== 'age')[0] || cols[0]).id;
    tbody.innerHTML = rows.map(r => {
      const sel = S.sel.has(r.id);
      return '<tr tabindex="0" data-id="' + esc(r.id) + '" class="' + (S.open.has(r.id) ? 'parent-open ' : '') + (sel ? 'checked ' : '') + (S.inspect === r.id ? 'selected' : '') + '">' +
        '<td class="col-check" data-label="Select"><input type="checkbox" class="row-check" aria-label="Select ' + esc(r.name) + '"' + (sel ? ' checked' : '') + '></td>' +
        cols.map((c, ci) => '<td data-label="' + c.label + '"' + (c.id === mob ? ' data-mob' : '') + (ci === 0 ? ' data-first' : '') + '><span class="cell-value' + (ci === 0 ? ' row-first' : '') + '">' + (ci === 0 ? firstPrefix(r) : '') + c.html(r) + '</span></td>').join('') +
        '<td class="col-act" data-label="Actions"><button type="button" class="row-act" aria-label="Actions for ' + esc(r.name) + '" aria-haspopup="menu">' + ICON('kebab') + '</button></td></tr>' + (S.open.has(r.id) ? treeRows(kidsOf(S.kind, r), 1, r.id) : '');
    }).join('');
    $('#wlEmpty').hidden = rows.length > 0;
    $('#rowCount').innerHTML = 'Showing ' + cnt(rows.length) + ' of ' + cnt(rowsOf().length) + ' ' + esc(K.label.toLowerCase());
    const all = $('#selAll');
    if (all) {
      const n = rows.filter(r => S.sel.has(r.id)).length;
      all.checked = rows.length > 0 && n === rows.length;
      all.indeterminate = n > 0 && n < rows.length;
    }
    renderBulk();
    updateFlOrb(rows.length);
    renderSummary();
  }

  function renderBulk(){
    bulkBar.hidden = S.sel.size === 0;
    $('#bulkCount').innerHTML = cnt(S.sel.size) + ' selected';
  }
  function updateCounts(){
    if (typeof updateNsPill === "function") updateNsPill();
    $$('[data-count]').forEach(el => { const k = el.dataset.count; el.textContent = k === 'namespaces' && S.ns.size ? S.ns.size : DATA[k].filter(inNs).length; });
  }
  function renderWorkloads(){
    const K = KINDS[S.kind];
    $('#wlEyebrow').textContent = K.group;
    $('#wlTitle').textContent = K.label;
    $('#wlSub').textContent = K.sub;
    $('#createLabel').textContent = 'Create ' + (K.one.length > 14 ? 'resource' : K.one);
    $('#createBtn').hidden = S.kind === 'events';
    search.placeholder = 'Filter ' + K.label.toLowerCase() + '…';
    $('#crumbCurrent').textContent = K.label;
    $('#wlBack').hidden = !S.from;
    $('#treeBar').hidden = !TREE_KINDS.has(S.kind);
    renderHead(); renderChips(); renderRows(); updateCounts();
  }
  function setKind(kind){
    let first = false;
    if (kind && kind !== S.kind) {
      S.kind = kind; S.q = ''; search.value = ''; S.sel.clear();
      closeInspector();
      if (!visited.has(kind)) { visited.add(kind); first = AUTO_EXPAND.has(kind); }
    }
    renderWorkloads();
    if (first) expandAll();
  }

  chipsEl.addEventListener('click', e => {
    const b = e.target.closest('.chip');
    if (!b) return;
    if (b.dataset.flt) { S.flt = null; renderChips(); renderRows(); return; }
    if (b.dataset.evtype) { const t = b.dataset.evtype; S.flt = t === 'all' ? null : {label:t === 'Warning' ? 'Warnings' : t, evtype:t, fn:r => r.type === t}; renderChips(); renderRows(); return; }
    if (b.dataset.nsopen !== undefined) { if (!nsMenu.hidden) closePops(); else openNsMenu(b); return; }
    if (b.dataset.nsx) S.ns.delete(b.dataset.nsx);
    else if (b.dataset.nsclear !== undefined) S.ns.clear();
    commitNs();
  });
  const colMenu = $('#colMenu');
  // Every change applies and is saved straight away; there is nothing to confirm.
  const saveCols = () => { lsWrite('openkite-cols', S.colHide); lsWrite('openkite-colorder', S.colOrder); lsWrite('openkite-sort', S.sortPref); };
  function colMenuHtml(){
    const K = KINDS[S.kind], hide = hiddenOf(S.kind), cols = orderedCols(K);
    const customised = S.colHide[S.kind] || S.colOrder[S.kind] || S.sortPref[S.kind];
    return '<div class="nsm-actions"><b class="col-title">Columns</b><button type="button" data-colreset' + (customised ? '' : ' disabled') + '>Reset</button></div><div class="nsm-list col-list" role="group" aria-label="Visible columns, in display order">' +
      cols.map((c, i) => '<div class="col-row' + (i === 0 ? ' locked' : '') + '" data-colrow="' + esc(c.id) + '">' +
        (i === 0 ? '<span class="col-grip off" aria-hidden="true"></span>' : '<button type="button" class="col-grip" data-colgrip aria-label="Drag to reorder ' + esc(c.label) + '. Arrow keys also move it" title="Drag to reorder"><svg class="icon" viewBox="0 0 16 16" aria-hidden="true"><path d="M5 3.5h.01M5 8h.01M5 12.5h.01M11 3.5h.01M11 8h.01M11 12.5h.01" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" fill="none"/></svg></button>') +
        '<button type="button" class="rm-item nf-item" role="menuitemcheckbox" aria-checked="' + (hide.indexOf(c.id) < 0) + '" data-col="' + esc(c.id) + '"' + (i === 0 ? ' disabled' : '') + '><span class="nf-box"></span><span class="rm-text">' + esc(c.label) + '</span>' + (i === 0 ? '<small>always first</small>' : '') + '</button></div>').join('') + '</div>' +
      '<div class="nsm-foot">Drag ⠿ to reorder · saved for ' + esc(K.label.toLowerCase()) + '</div>';
  }
  const refreshColMenu = () => {
    const list = $('.col-list', colMenu), st = list ? list.scrollTop : 0;
    colMenu.innerHTML = colMenuHtml(); const l2 = $('.col-list', colMenu); if (l2) l2.scrollTop = st;
    renderHead(); renderRows();
  };
  $('#colBtn').addEventListener('click', e => {
    if (!colMenu.hidden) return void closePops();
    closePops(); colMenu.innerHTML = colMenuHtml(); placePop(colMenu, e.currentTarget, true); e.currentTarget.setAttribute('aria-expanded', 'true');
  });
  if (window.matchMedia) { const mq = window.matchMedia('(max-width:767px)'); const shut = () => { if (mq.matches && !colMenu.hidden) closePops(); }; if (mq.addEventListener) mq.addEventListener('change', shut); }
  const moveCol = (id, to) => {
    const ids = orderedCols(KINDS[S.kind]).map(c => c.id).slice(1), i = ids.indexOf(id);
    to = Math.max(0, Math.min(ids.length - 1, to));
    if (i < 0 || i === to) return false;
    ids.splice(to, 0, ids.splice(i, 1)[0]); S.colOrder[S.kind] = ids; saveCols(); return true;
  };
  colMenu.addEventListener('click', e => {
    if (e.target.closest('[data-colgrip]')) return;
    const it = e.target.closest('[data-col]'), rs = e.target.closest('[data-colreset]');
    if (it && !it.disabled) {
      const hide = hiddenOf(S.kind).slice(), i = hide.indexOf(it.dataset.col);
      if (i >= 0) hide.splice(i, 1); else hide.push(it.dataset.col);
      S.colHide[S.kind] = hide;
    } else if (rs && !rs.disabled) { delete S.colHide[S.kind]; delete S.colOrder[S.kind]; delete S.sortPref[S.kind]; }
    else return;
    saveCols(); refreshColMenu();
  });
  // keyboard: arrows on the grip move the column
  colMenu.addEventListener('keydown', e => {
    const g = e.target.closest && e.target.closest('[data-colgrip]');
    if (!g || (e.key !== 'ArrowUp' && e.key !== 'ArrowDown')) return;
    e.preventDefault();
    const id = g.closest('.col-row').dataset.colrow, ids = orderedCols(KINDS[S.kind]).map(c => c.id).slice(1);
    if (moveCol(id, ids.indexOf(id) + (e.key === 'ArrowUp' ? -1 : 1))) { refreshColMenu(); const n = $('.col-row[data-colrow="' + CSS.escape(id) + '"] [data-colgrip]', colMenu); if (n) n.focus(); }
  });
  // pointer drag: the row follows the pointer, neighbours swap out of its way, the new order is saved on release
  let cd = null;
  colMenu.addEventListener('pointerdown', e => {
    const g = e.target.closest && e.target.closest('[data-colgrip]');
    if (!g || (e.pointerType === 'mouse' && e.button !== 0)) return;
    const row = g.closest('.col-row'), list = row.parentNode;
    cd = {row, list, id:row.dataset.colrow, y0:e.clientY, pid:e.pointerId, moved:false};
    try { g.setPointerCapture(e.pointerId); } catch (err) { /* not supported */ }
    e.preventDefault();
  });
  colMenu.addEventListener('pointermove', e => {
    if (!cd || e.pointerId !== cd.pid) return;
    const dy = e.clientY - cd.y0;
    if (!cd.moved && Math.abs(dy) < 4) return;
    if (!cd.moved) { cd.moved = true; cd.row.classList.add('dragging'); cd.list.classList.add('is-dragging'); }
    const rows = Array.from(cd.list.children).filter(r => r !== cd.row);
    let before = null;
    for (const r of rows.slice(1)) { const rr = r.getBoundingClientRect(); if (e.clientY < rr.top + rr.height / 2) { before = r; break; } }
    if (before !== cd.row.nextElementSibling) { const was = cd.row.offsetTop; cd.list.insertBefore(cd.row, before); cd.y0 += cd.row.offsetTop - was; }
    cd.row.style.transform = 'translateY(' + (e.clientY - cd.y0) + 'px)';
  });
  const endColDrag = e => {
    if (!cd || (e && e.pointerId !== cd.pid)) return;
    const d = cd; cd = null;
    d.row.classList.remove('dragging'); d.list.classList.remove('is-dragging'); d.row.style.transform = '';
    if (!d.moved) return;
    const ids = Array.from(d.list.children).map(r => r.dataset.colrow).slice(1);
    S.colOrder[S.kind] = ids; saveCols(); refreshColMenu();
  };
  colMenu.addEventListener('pointerup', endColDrag);
  colMenu.addEventListener('pointercancel', endColDrag);
  search.addEventListener('input', () => { S.q = search.value; renderRows(); });
  search.addEventListener('keydown', e => { if (e.key === 'Escape' && search.value) { e.preventDefault(); $('#flClear').click(); } });
  $('#flClear').addEventListener('click', () => { S.q = ''; search.value = ''; renderRows(); search.focus(); });
  head.addEventListener('click', e => {
    const ch = e.target.closest('.col-check');
    if (ch && e.target.tagName !== 'INPUT') { const i = $('input', ch); if (i) { i.checked = !i.checked; i.dispatchEvent(new Event('change', {bubbles:true})); } return; }
    const b = e.target.closest('.th-sort');
    if (!b) return;
    // asc -> desc -> back to the default (name A-Z); whatever the user picks is remembered per resource type
    const id = b.dataset.sort, cur = curSort();
    if (cur.id !== id) S.sortPref[S.kind] = {id, dir:1};
    else if (cur.dir > 0) S.sortPref[S.kind] = {id, dir:-1};
    else delete S.sortPref[S.kind];
    const ds = defaultSort(S.kind), sp = S.sortPref[S.kind];
    if (sp && sp.id === ds.id && sp.dir === ds.dir) delete S.sortPref[S.kind];
    lsWrite('openkite-sort', S.sortPref);
    renderHead(); renderRows();
  });
  head.addEventListener('change', e => {
    if (e.target.id !== 'selAll') return;
    visibleRows().forEach(r => { if (e.target.checked) S.sel.add(r.id); else S.sel.delete(r.id); });
    renderRows();
  });
  tbody.addEventListener('click', e => {
    if (e.target.closest('[data-ns-filter]')) return;   // namespace text filters; it must not open the row
    const tr = e.target.closest('tr[data-id]');
    if (!tr) return;
    const id = tr.dataset.id;
    if (e.target.closest('.row-check')) {
      if (e.target.checked) S.sel.add(id); else S.sel.delete(id);
      tr.classList.toggle('checked', e.target.checked);
      renderBulk();
      return;
    }
    if (e.target.closest('.col-check')) {
      const cb = $('.row-check', tr);
      cb.checked = !cb.checked;
      if (cb.checked) S.sel.add(id); else S.sel.delete(id);
      tr.classList.toggle('checked', cb.checked);
      renderBulk();
      return;
    }
    const act = e.target.closest('.row-act');
    if (act) { openRowMenu(id, act); return; }
    navClear(); openInspector(id);
  });
  tbody.addEventListener('keydown', e => {
    const tr = e.target.closest('tr[data-id]');
    if (tr && e.target === tr && (e.key === 'Enter' || e.key === ' ')) { e.preventDefault(); openInspector(tr.dataset.id); }
  });
  $('#bulkClear').addEventListener('click', () => { S.sel.clear(); renderRows(); });
  $('#bulkDelete').addEventListener('click', () => {
    const items = Array.from(S.sel).map(byId).filter(Boolean);
    confirmDlg({
      title:'Delete ' + items.length + ' resource' + (items.length === 1 ? '' : 's') + '?',
      html:'<p>This permanently removes the selected ' + esc(KINDS[S.kind].label.toLowerCase()) + ' from <span class="mono">prod-us-east-1</span>.</p>',
      ok:'Delete', danger:true,
      onOk:() => { items.forEach(it => removeResource(it.r.id, true)); S.sel.clear(); renderWorkloads(); showToast(items.length + ' deleted'); }
    });
  });

  /* ------------------------------------------------------------ row menu */
  let menuFor = null;
  function openRowMenu(id, anchor){
    const f = byId(id);
    if (!f) return;
    menuFor = id;
    rowMenu.innerHTML = KINDS[f.kind].actions.map(a => '<button type="button" role="menuitem" class="rm-item' + (ACTION[a].danger ? ' danger' : '') + '" data-act="' + a + '">' + ICON(ACTION[a].icon) + ACTION[a].label + '</button>').join('');
    rowMenu.hidden = false;
    const r = anchor.getBoundingClientRect(), w = rowMenu.offsetWidth, hgt = rowMenu.offsetHeight;
    rowMenu.style.left = Math.max(8, Math.min(window.innerWidth - w - 8, r.right - w)) + 'px';
    rowMenu.style.top = (r.bottom + hgt + 8 > window.innerHeight ? Math.max(8, r.top - hgt - 4) : r.bottom + 4) + 'px';
    const first = $('.rm-item', rowMenu); if (first) first.focus();
  }
  function closeRowMenu(){ rowMenu.hidden = true; menuFor = null; }
  rowMenu.addEventListener('click', e => {
    const b = e.target.closest('.rm-item');
    if (!b) return;
    const id = menuFor;
    closeRowMenu();
    runAction(b.dataset.act, id);
  });
  document.addEventListener('click', e => {
    if (!rowMenu.hidden && !e.target.closest('#rowMenu') && !e.target.closest('.row-act')) closeRowMenu();
  });
  main.addEventListener('scroll', closeRowMenu, {passive:true});

  /* ------------------------------------------------------------- actions */
  function runAction(act, id){
    const f = byId(id);
    if (!f) return;
    const r = f.r;
    if (act === 'logs') {
      if (f.kind === 'pods') return openLog(r);
      const p = podsOf(f.kind, r)[0];
      return p ? openLog(p) : showToast('No running pods for ' + r.name);
    }
    if (act === 'shell') return openShell(r);
    if (act === 'resources') return gotoResource(id, 'resources');
    if (act === 'edit') return startEdit(id);
    if (act === 'copy') return copyText(r.name, 'Copied ' + r.name);
    if (act === 'forward') {
      return confirmDlg({
        title:'Port forward ' + r.name,
        html:'<label class="field"><span>Local port</span><input type="number" id="fwdPort" min="1" max="65535" value="8080"></label><p>Traffic to <span class="mono">localhost</span> is tunnelled to <span class="mono">' + esc(r.name) + '</span>.</p>',
        ok:'Start forwarding',
        onOk:() => showToast('Forwarding localhost:' + ($('#fwdPort').value || 8080) + ' → ' + r.name)
      });
    }
    if (act === 'cordon') { r.status = /Disabled/.test(r.status) ? 'Ready' : 'Ready,SchedulingDisabled'; renderRows(); if (S.inspect === id) renderInspector(id); return showToast(r.name + (/Disabled/.test(r.status) ? ' cordoned' : ' uncordoned')); }
    if (act === 'drain') return showToast('Draining ' + r.name + '…');
    if (act === 'rollback') return showToast('Rolled ' + r.name + ' back to revision ' + Math.max(1, r.revision - 1));
    if (act === 'restart') return showToast('Rollout restart triggered for ' + r.name);
    if (act === 'trigger') return showToast('Job created from ' + r.name);
    if (act === 'scale') {
      return confirmDlg({
        title:'Scale ' + r.name,
        html:'<label class="field"><span>Replicas</span><input type="number" id="scaleInput" min="0" max="50" value="' + r.want + '"></label>',
        ok:'Scale',
        onOk:() => {
          const n = Math.max(0, Math.min(50, parseInt($('#scaleInput').value, 10) || 0));
          r.want = n; r.have = n;
          renderWorkloads(); if (S.inspect === id) renderInspector(id);
          showToast(r.name + ' scaled to ' + n);
        }
      });
    }
    if (act === 'delete') {
      confirmDlg({
        title:'Delete ' + KINDS[f.kind].one.toLowerCase() + '?',
        html:'<p><span class="mono">' + esc(r.name) + '</span> will be removed from <span class="mono">' + esc(r.ns) + '</span>. This can’t be undone.</p>',
        ok:'Delete', danger:true,
        onOk:() => { removeResource(id); showToast(KINDS[f.kind].one + ' ' + r.name + ' deleted'); }
      });
    }
  }
  function removeResource(id, silent){
    const f = byId(id);
    if (!f) return;
    DATA[f.kind] = DATA[f.kind].filter(x => x.id !== id);
    S.sel.delete(id);
    const label = OWNER_LABEL[f.kind];
    if (label) DATA.pods.filter(p => p.ns === f.r.ns && p.ctrl === label + '/' + f.r.name).forEach(p => removeResource(p.id, true));
    dropTabsFor(id);
    if (S.inspect === id) closeInspector();
    if (!silent) renderWorkloads();
  }

  /* -------------------------------------------------------------- dialog */
  const dlgScrim = $('#dlgScrim'), dlgOk = $('#dlgOk');
  let dlgOkFn = null, dlgReturn = null;
  function confirmDlg(o){
    $('#dlgTitle').textContent = o.title;
    $('#dlgBody').innerHTML = o.html;
    dlgOk.textContent = o.ok;
    dlgOk.className = 'btn ' + (o.danger ? 'btn-danger' : 'btn-primary');
    dlgOkFn = o.onOk;
    dlgReturn = document.activeElement;
    dlgScrim.hidden = false;
    $('.dlg', dlgScrim).classList.toggle('wide', !!o.wide);
    ($('#dlgBody input') || $('#dlgCancel')).focus();
  }
  function closeDlg(){ dlgScrim.hidden = true; dlgOkFn = null; if (dlgReturn && dlgReturn.focus) dlgReturn.focus(); }
  dlgOk.addEventListener('click', () => { const fn = dlgOkFn; closeDlg(); if (fn) fn(); });
  $('#dlgCancel').addEventListener('click', closeDlg);
  dlgScrim.addEventListener('click', e => { if (e.target === dlgScrim) closeDlg(); });
  dlgScrim.addEventListener('keydown', e => { if (e.key === 'Enter' && e.target.tagName === 'INPUT') { e.preventDefault(); dlgOk.click(); } });

  $('#createBtn').addEventListener('click', () => {
    const K = KINDS[S.kind], kind = S.kind;
    const nsList = DATA.namespaces.map(n => n.name);
    confirmDlg({
      title:'Create ' + K.one,
      html:'<label class="field"><span>Name</span><input type="text" id="newName" placeholder="my-' + K.one.toLowerCase() + '" autocomplete="off" spellcheck="false"></label>' +
        (K.cluster ? '' : '<label class="field"><span>Namespace</span><select id="newNs">' + nsList.map(n => '<option>' + esc(n) + '</option>').join('') + '</select></label>'),
      ok:'Create',
      onOk:() => {
        const name = ($('#newName').value || '').trim().toLowerCase().replace(/[^a-z0-9-]/g, '-') || 'new-' + K.one.toLowerCase();
        const ns = K.cluster ? undefined : $('#newNs').value;
        let r;
        if (kind === 'pods') {
          r = pod(name, ns, ['main'], 0, 'Pod/' + name, 'ip-10-0-4-12', 'BestEffort', 1, 'ContainerCreating', '10.244.4.99');
          setTimeout(() => { r.status = 'Running'; r.age = 4; if (S.kind === 'pods') renderRows(); }, 1800);
        } else if (G[kind]) {
          r = Object.assign({}, DATA[kind][0], {name, age:1});
          if (ns) r.ns = ns;
          r.id = kind + '/' + (ns || '-') + '/' + name;
        } else {
          r = {id:ID_PREFIX[kind] + '/' + ns + '/' + name, name, ns, want:1, have:0, age:1, state:'Running', schedule:'0 * * * *', suspend:false, last:0};
          setTimeout(() => { r.have = r.want; if (S.kind === kind) renderRows(); }, 1800);
        }
        DATA[kind].unshift(r);
        renderWorkloads();
        showToast(K.one + ' ' + name + ' created');
      }
    });
  });

  /* ----------------------------------------------------------- inspector */
  const API_OF = {pods:'v1', deployments:'apps/v1', daemonsets:'apps/v1', statefulsets:'apps/v1', jobs:'batch/v1', cronjobs:'batch/v1'};
  const iso = age => new Date(Date.UTC(2025, 5, 12, 10, 40, 0) - (age || 0) * 1000).toISOString().replace('.000Z', 'Z');
  const baseName = n => n.replace(/-[a-f0-9]{9,10}-[a-z0-9]{5}$/, '').replace(/-[a-z0-9]{5}$/, '').replace(/-\d+$/, '');
  const hashOf = s => Array.from(s).reduce((a, c) => (a * 31 + c.charCodeAt(0)) >>> 0, 7);
  const imageOf = c => 'ghcr.io/acme/' + c + ':1.' + (hashOf(c) % 9 + 2) + '.' + (hashOf(c) % 7);
  function labelsFor(kind, r){
    const l = {'app.kubernetes.io/name':baseName(r.name), 'app.kubernetes.io/part-of':'prod-us-east-1'};
    if (kind === 'pods') { const m = r.name.match(/-([a-f0-9]{9,10})-/); if (m) l['pod-template-hash'] = m[1]; }
    if (r.ns === 'argocd') l['app.kubernetes.io/managed-by'] = 'argocd';
    return l;
  }
  function ownerOf(r){
    if (!r.ctrl) return null;
    const i = r.ctrl.indexOf('/'), label = r.ctrl.slice(0, i), name = r.ctrl.slice(i + 1);
    const k = Object.keys(OWNER_LABEL).find(x => OWNER_LABEL[x] === label);
    const hit = k && DATA[k].find(x => x.ns === r.ns && x.name === name);
    return hit ? {id:hit.id, label:r.ctrl} : null;
  }
  /* Scheduling constraints live on the pod template; only the ones that are actually set are returned */
  const SCHED_KINDS = new Set(['pods', 'deployments', 'statefulsets', 'daemonsets', 'replicasets', 'jobs', 'cronjobs']);
  function schedOf(kind, r){
    if (!SCHED_KINDS.has(kind)) return null;
    const key = (kind === 'pods' || kind === 'replicasets' ? baseName(r.name) : r.name), h = hashOf(r.ns + '/' + key), out = {};
    if (kind === 'daemonsets') out.tolerations = [{key:'node-role.kubernetes.io/control-plane', operator:'Exists', effect:'NoSchedule'}, {key:'node.kubernetes.io/not-ready', operator:'Exists', effect:'NoExecute'}];
    else if (h % 4 === 1) out.tolerations = [{key:'dedicated', operator:'Equal', value:'workloads', effect:'NoSchedule'}];
    if (h % 3 !== 2) out.nodeSelector = h % 5 === 0 ? {'kubernetes.io/os':'linux', 'node.kubernetes.io/pool':'general'} : {'kubernetes.io/os':'linux'};
    if (h % 3 === 0) out.affinity = {podAntiAffinity:{preferredDuringSchedulingIgnoredDuringExecution:[{weight:100, podAffinityTerm:{topologyKey:'kubernetes.io/hostname', labelSelector:{matchLabels:{'app.kubernetes.io/name':key.replace(/-\d+$/, '')}}}}]}};
    else if (h % 7 === 1) out.affinity = {nodeAffinity:{requiredDuringSchedulingIgnoredDuringExecution:{nodeSelectorTerms:[{matchExpressions:[{key:'topology.kubernetes.io/zone', operator:'In', values:['us-east-1a', 'us-east-1b']}]}]}}};
    return Object.keys(out).length ? out : null;
  }
  function schedHtml(kind, r){
    const sc = schedOf(kind, r);
    if (!sc) return '';
    let h = '<div class="inspector-eyebrow">Scheduling</div>';
    if (sc.nodeSelector) h += '<div class="sched-block"><div class="sched-title">Node selector</div>' + chipsOf(sc.nodeSelector) + '</div>';
    if (sc.tolerations) h += '<div class="sched-block"><div class="sched-title">Tolerations ' + cnt(sc.tolerations.length) + '</div><ul class="sched-list">' + sc.tolerations.map(t =>
      '<li><span class="mono">' + esc(t.key + (t.operator === 'Equal' ? '=' + t.value : '')) + '</span><span class="sched-tag">' + esc(t.effect) + '</span></li>').join('') + '</ul></div>';
    if (sc.affinity) {
      const rows = [], A = sc.affinity;
      if (A.podAntiAffinity) rows.push(['Pod anti-affinity', 'Prefer spreading across ' + A.podAntiAffinity.preferredDuringSchedulingIgnoredDuringExecution[0].podAffinityTerm.topologyKey, 'preferred']);
      if (A.nodeAffinity) A.nodeAffinity.requiredDuringSchedulingIgnoredDuringExecution.nodeSelectorTerms.forEach(t => t.matchExpressions.forEach(m => rows.push(['Node affinity', m.key + ' ' + m.operator + ' ' + m.values.join(', '), 'required'])));
      h += '<div class="sched-block"><div class="sched-title">Affinity ' + cnt(rows.length) + '</div><ul class="sched-list">' + rows.map(x => '<li><span><b>' + esc(x[0]) + '</b> <span class="mono">' + esc(x[1]) + '</span></span><span class="sched-tag">' + x[2] + '</span></li>').join('') + '</ul></div>';
    }
    return h;
  }
  function manifest(kind, r){
    const K = KINDS[kind], meta = {name:r.name};
    if (!K.cluster) meta.namespace = r.ns;
    meta.creationTimestamp = iso(r.age);
    meta.labels = labelsFor(kind, r);
    let spec, status;
    if (kind === 'pods') {
      const o = r.ctrl.split('/');
      meta.ownerReferences = [{kind:o[0], name:o[1], controller:true}];
      spec = {nodeName:r.node, restartPolicy:'Always', containers:r.ctrs.map((c, i) => ({name:c, image:imageOf(c), ports:[{containerPort:i ? 15001 : 8080, protocol:'TCP'}], resources:{requests:{cpu:'100m', memory:'128Mi'}, limits:{cpu:'500m', memory:'512Mi'}}}))};
      status = {phase:r.status === 'Running' ? 'Running' : r.status === 'Completed' ? 'Succeeded' : 'Pending', podIP:r.ip, qosClass:r.qos, containerStatuses:r.ctrs.map(c => ({name:c, ready:r.status === 'Running', restartCount:r.restarts}))};
    } else if (kind === 'deployments' || kind === 'daemonsets' || kind === 'statefulsets') {
      spec = {replicas:r.want, selector:{matchLabels:{'app.kubernetes.io/name':baseName(r.name)}}};
      status = {replicas:r.want, readyReplicas:r.have};
    } else if (kind === 'jobs') { spec = {completions:r.want, backoffLimit:3}; status = {succeeded:r.have, conditions:[{type:r.state}]}; }
    else if (kind === 'cronjobs') { spec = {schedule:r.schedule, suspend:r.suspend}; status = {lastScheduleTime:iso(r.last)}; }
    else { spec = {}; Object.keys(r).forEach(k => { if (['id', 'name', 'ns', 'age'].indexOf(k) < 0) spec[k] = r[k]; }); }
    const sc = schedOf(kind, r);
    if (sc) { if (kind === 'pods') Object.assign(spec, sc); else if (kind === 'cronjobs') spec.jobTemplate = {spec:{template:{spec:sc}}}; else spec.template = {spec:sc}; }
    return {apiVersion:API_OF[kind] || K.api || 'v1', kind:K.one, metadata:meta, spec, status};
  }
  const yScalar = v => typeof v === 'string' && (/[:#{}\[\],&*!|>'"%@`]|^\s|\s$|^$|^(true|false|null|~|[0-9])/i.test(v)) ? JSON.stringify(v) : String(v);
  function toYaml(o, ind){
    const pad = '  '.repeat(ind || 0);
    return Object.keys(o).filter(k => o[k] !== undefined && o[k] !== null).map(k => {
      const v = o[k];
      if (Array.isArray(v)) {
        if (!v.length) return pad + k + ': []';
        return pad + k + ':\n' + v.map(it => {
          if (it && typeof it === 'object') { const ls = toYaml(it, (ind || 0) + 1).split('\n'); ls[0] = pad + '- ' + ls[0].trimStart(); return ls.join('\n'); }
          return pad + '- ' + yScalar(it);
        }).join('\n');
      }
      if (v && typeof v === 'object') return Object.keys(v).length ? pad + k + ':\n' + toYaml(v, (ind || 0) + 1) : pad + k + ': {}';
      return pad + k + ': ' + yScalar(v);
    }).join('\n');
  }
  /* ---- YAML syntax highlighting + linter (indentation, structure, quoting, duplicate keys and Kubernetes field checks) */
  function yamlLineHtml(line){
    let body = line, tail = '';
    const ci = line.search(/\s#/);
    if (ci >= 0) { const before = line.slice(0, ci); if ((before.match(/"/g) || []).length % 2 === 0 && (before.match(/'/g) || []).length % 2 === 0) { body = before; tail = line.slice(ci); } }
    if (/^\s*#/.test(line)) return '<span class="y-c">' + esc(line) + '</span>';
    const m = body.match(/^(\s*(?:-\s+)*)((?:"[^"]*"|'[^']*'|[^'"#\s][^#]*?))(\s*):(?:(\s+)(.*))?$/);
    const scalar = v => /^(-?\d+(\.\d+)?|0x[0-9a-f]+)$/i.test(v) ? 'y-n' : /^(true|false|yes|no|on|off)$/i.test(v) ? 'y-b' : /^(null|~)$/.test(v) ? 'y-z' : /^["']/.test(v) ? 'y-q' : /^[&*!]/.test(v) ? 'y-a' : 'y-s';
    let out;
    if (m) out = esc(m[1]).replace(/-/g, '<span class="y-d">-</span>') + '<span class="y-k">' + esc(m[2]) + '</span>' + esc(m[3]) + '<span class="y-d">:</span>' + (m[5] != null ? esc(m[4] || '') + '<span class="' + scalar(m[5].trim()) + '">' + esc(m[5]) + '</span>' : '');
    else { const d = body.match(/^(\s*)(-\s+)(.*)$/); out = d ? esc(d[1]) + '<span class="y-d">-</span> <span class="' + scalar(d[3].trim()) + '">' + esc(d[3]) + '</span>' : '<span class="y-s">' + esc(body) + '</span>'; }
    return out + (tail ? '<span class="y-c">' + esc(tail) + '</span>' : '');
  }
  const yamlHtml = text => text.split('\n').map(yamlLineHtml).join('\n');
  const Q_CPU = /^\d+(\.\d+)?m?$/, Q_MEM = /^\d+(\.\d+)?(E|P|T|G|M|k|K|Ei|Pi|Ti|Gi|Mi|Ki)?$/;
  function yamlLint(text, f){
    const out = [], lines = text.split('\n');
    const err = (i, c, m) => out.push({line:i + 1, col:c + 1, sev:'error', msg:m}), warn = (i, c, m) => out.push({line:i + 1, col:c + 1, sev:'warn', msg:m});
    if (!text.trim()) { err(0, 0, 'The manifest is empty.'); return out; }
    const stack = [{indent:-1, keys:new Map()}], path = [];
    let block = -1, prevOpens = false, first = true, kindAt = -1, nameAt = -1, metaIndent = -1, seen = {apiVersion:false, kind:false, metadata:false};
    for (let i = 0; i < lines.length; i++) {
      const raw = lines[i], lead = raw.match(/^\s*/)[0];
      if (/\t/.test(lead)) { err(i, 0, 'Tab characters are not allowed for indentation. Use spaces.'); continue; }
      if (!raw.trim()) continue;
      const indent = lead.length, body = raw.slice(indent);
      if (block >= 0) { if (indent > block) continue; block = -1; }
      if (body[0] === '#') continue;
      if (body === '---' || body === '...') continue;
      if (/\s+$/.test(raw)) warn(i, raw.replace(/\s+$/, '').length, 'Trailing whitespace.');
      if (first && indent > 0) err(i, 0, 'The document must start at the first column.');
      if (indent % 2) warn(i, 0, 'Indentation is not a multiple of 2 spaces.');
      const top = stack[stack.length - 1];
      if (indent > top.indent) {
        if (!prevOpens && !first) err(i, 0, 'Unexpected indentation: the previous line does not open a block (it is missing a ":" or is a complete value).');
        stack.push({indent, keys:new Map()});
      } else {
        while (stack.length > 1 && stack[stack.length - 1].indent > indent) stack.pop();
        if (stack[stack.length - 1].indent !== indent) { err(i, 0, 'Inconsistent indentation: this line does not line up with any parent level.'); stack.push({indent, keys:new Map()}); }
      }
      first = false;
      let rest = body, col = indent, isItem = false;
      const dash = rest.match(/^-(\s+|$)/);
      if (dash) {
        isItem = true; rest = rest.slice(dash[0].length); col += dash[0].length;
        while (stack.length > 1 && stack[stack.length - 1].indent > indent) stack.pop();
        stack.push({indent:col, keys:new Map()});
        if (!rest) { prevOpens = true; continue; }
      }
      const scope = stack[stack.length - 1];
      const m = rest.match(/^("(?:[^"\\]|\\.)*"|'(?:[^']|'')*'|[^'"#\s][^#]*?)\s*:(?:\s+(.*)|$)/);
      if (!m) {
        prevOpens = false;
        if (isItem) { checkScalar(rest.replace(/\s+#.*$/, ''), i, col); continue; }
        if (/^[^\s#]+:[^\s]/.test(rest)) err(i, col, 'Missing a space after ":". Write "key: value".');
        else err(i, col, 'Expected "key: value" or a "- list item".');
        continue;
      }
      const key = m[1].replace(/^["']|["']$/g, ''), valRaw = (m[2] || '');
      let val = valRaw.trim();
      if (!/^["']/.test(val)) val = val.replace(/\s+#.*$/, '').trim();
      else { const qm = val.match(/^("(?:[^"\\]|\\.)*"|'(?:[^']|'')*')\s*(?:#.*)?$/); if (qm) val = qm[1]; }
      if (scope.keys.has(key)) err(i, col, 'Duplicate key "' + key + '" (first defined on line ' + scope.keys.get(key) + ').'); else scope.keys.set(key, i + 1);
      prevOpens = val === '' || /^[&!][^\s]*$/.test(val);
      if (/^[|>][+-]?\d*$/.test(val)) { block = indent + (isItem ? 2 : 0); prevOpens = false; }
      else if (val !== '') checkValue(key, val, i, col + m[1].length + 1);
      if (indent === 0 && !isItem) {
        if (key === 'apiVersion') seen.apiVersion = true;
        if (key === 'kind') { seen.kind = true; kindAt = i; }
        if (key === 'metadata') { seen.metadata = true; metaIndent = 0; }
      }
      if (key === 'name' && nameAt < 0 && indent === 2 && seen.metadata) nameAt = i;
    }
    function checkScalar(v, i, c){ if (/^["']/.test(v) && !closedQuote(v)) err(i, c, 'Unclosed quote.'); }
    function closedQuote(v){ const q = v[0]; return q === '"' ? /^"(?:[^"\\]|\\.)*"$/.test(v) : /^'(?:[^']|'')*'$/.test(v); }
    function checkValue(key, v, i, c){
      if (/^["']/.test(v)) { if (!closedQuote(v)) err(i, c, 'Unclosed ' + (v[0] === '"' ? 'double' : 'single') + ' quote.'); v = v.slice(1, -1); }
      else {
        if (/^[\[{]/.test(v)) { const open = v[0], close = open === '[' ? ']' : '}'; if (v[v.length - 1] !== close || v.split(open).length !== v.split(close).length) err(i, c, 'Unclosed "' + open + '": expected a closing "' + close + '".'); return; }
        if (/^[@\`]/.test(v)) err(i, c, 'A plain value cannot start with "' + v[0] + '". Quote it.');
        else if (/:\s/.test(v) || /\s#/.test(v)) err(i, c, 'An unquoted value cannot contain ": ". Wrap it in quotes.');
        if (/^(yes|no|on|off)$/i.test(v)) warn(i, c, '"' + v + '" is read as a string in YAML 1.2. Use true or false for booleans.');
      }
      const int = /^\d+$/.test(v);
      if (/^(replicas|completions|parallelism|backoffLimit|minReplicas|maxReplicas)$/.test(key)) { if (!int) err(i, c, key + ' must be a whole number.'); else if (Number(v) > 50) warn(i, c, key + ' is above 50; this console caps it at 50.'); }
      else if (/^(containerPort|port|nodePort|hostPort)$/.test(key) && (!int || Number(v) < 1 || Number(v) > 65535)) err(i, c, key + ' must be a number between 1 and 65535.');
      else if (key === 'cpu' && !Q_CPU.test(v)) err(i, c, 'Invalid CPU quantity "' + v + '". Use e.g. 250m or 1.');
      else if ((key === 'memory' || key === 'ephemeral-storage') && !Q_MEM.test(v)) err(i, c, 'Invalid quantity "' + v + '". Use e.g. 128Mi or 1Gi.');
      else if (key === 'restartPolicy' && !/^(Always|OnFailure|Never)$/.test(v)) err(i, c, 'restartPolicy must be Always, OnFailure or Never.');
      else if (key === 'protocol' && !/^(TCP|UDP|SCTP)$/.test(v)) err(i, c, 'protocol must be TCP, UDP or SCTP.');
      else if (key === 'imagePullPolicy' && !/^(Always|IfNotPresent|Never)$/.test(v)) err(i, c, 'imagePullPolicy must be Always, IfNotPresent or Never.');
    }
    if (!seen.apiVersion) err(0, 0, 'apiVersion is required.');
    if (!seen.kind) err(0, 0, 'kind is required.');
    if (!seen.metadata) err(0, 0, 'metadata is required.');
    if (f) {
      const kind = kindAt >= 0 ? (lines[kindAt].match(/^kind:\s*(.*?)\s*$/) || [])[1] : '';
      if (kind && kind.replace(/["']/g, '') !== KINDS[f.kind].one) err(kindAt, 0, 'kind is immutable. It must stay ' + KINDS[f.kind].one + '.');
      if (seen.metadata && nameAt < 0) err(0, 0, 'metadata.name is required.');
      else if (nameAt >= 0) { const nm = (lines[nameAt].match(/name:\s*(.*?)\s*$/) || [])[1].replace(/["']/g, ''); if (nm !== f.r.name) err(nameAt, 2, 'metadata.name is immutable. It must stay ' + f.r.name + '.'); }
    }
    return out.sort((a, b) => a.line - b.line || (a.sev === 'error' ? -1 : 1));
  }
  function eventsFor(r, kind){
    if (kind === 'events') return [[r.type, r.reason, r.message, fmtAge(r.age)]];
    if (kind !== 'pods') return [['Normal', 'Synced', 'Resource synced from desired state', fmtAge(r.age || 60)]];
    const ev = [['Normal', 'Scheduled', 'Assigned to ' + r.node, fmtAge(r.age)], ['Normal', 'Pulled', 'Container image already present on machine', fmtAge(r.age)], ['Normal', 'Started', 'Started container ' + r.ctrs[0], fmtAge(r.age)]];
    if (r.status === 'CrashLoopBackOff') ev.push(['Warning', 'BackOff', 'Back-off restarting failed container ' + r.ctrs[0], '6m']);
    if (r.status === 'Pending') ev.push(['Warning', 'FailedMount', 'MountVolume.SetUp failed for volume "webhook-tls"', '34m']);
    if (r.status === 'ContainerCreating') ev.push(['Normal', 'Pulling', 'Pulling image for ' + r.ctrs[0], '35m']);
    return ev.reverse();
  }
  const nsLink = ns => ns && ns !== '—' ? '<button type="button" class="ns-link" data-ns-filter="' + esc(ns) + '" title="Filter everything by namespace ' + esc(ns) + '">' + esc(ns) + '</button>' : esc(ns || '—');
  const kv = (k, v) => '<div class="kv-row"><dt>' + k + '</dt><dd>' + v + '</dd></div>';
  const chipsOf = o => '<div class="chip-list">' + Object.keys(o).map(k => '<span class="label-chip"><b>' + esc(k) + '</b>=' + esc(o[k]) + '</span>').join('') + '</div>';
  const meterRow = (label, used, limit, text) => '<div class="meter"><div class="meter-head"><span>' + label + '</span><span class="mono">' + text + '</span></div><div class="meter-track"><span class="meter-fill" style="width:' + Math.min(100, Math.round(used / limit * 100)) + '%"></span></div></div>';

  function overviewPanel(f){
    if (f.kind === 'crds') return crdOverview(f);
    const r = f.r, K = KINDS[f.kind];
    let props, extra = '';
    if (f.kind === 'pods') {
      const own = ownerOf(r);
      props = kv('Created', fmtAge(r.age) + ' ago') + kv('Namespace', nsLink(r.ns)) + kv('Node', '<span class="mono">' + esc(r.node) + '</span>') + kv('Pod IP', '<span class="mono">' + esc(r.ip) + '</span>') +
        kv('Controlled by', own ? '<button type="button" class="link-btn" data-goto="' + esc(own.id) + '">' + esc(own.label) + '</button>' : '<span class="mono">' + esc(r.ctrl) + '</span>') +
        kv('QoS class', esc(r.qos)) + kv('Restarts', r.restarts) + kv('Status', pill(r.status, podTone(r.status)));
      const m = metricsOf('pods', r), up = r.status === 'Running', F = OK.FMT, cpu = OK.last(m.cpu.data), mem = OK.last(m.mem.data), net = OK.last(m.rx) + OK.last(m.tx);
      extra = '<div class="inspector-eyebrow">Usage</div><div class="gauge-row">' + OK.gauge(up ? cpu / m.cpu.limit * 100 : 0, 'CPU', F.cpu(cpu), C.cpu) + OK.gauge(up ? mem / m.mem.limit * 100 : 0, 'Memory', F.mem(mem), C.mem) + OK.gauge(up ? net / 300 * 100 : 0, 'Network', F.bps(net), C.rx) + OK.gauge(m.fsPct, 'Disk', m.fsPct + '% used', C.fr) + '</div><button type="button" class="link-btn" data-tab-go="metrics">View metrics →</button>';
      const ok = r.status === 'Running' || r.status === 'Completed';
      extra += '<div class="inspector-eyebrow">Conditions</div><ul class="cond-list">' + ['PodScheduled', 'Initialized', 'ContainersReady', 'Ready'].map((c, i) =>
        '<li><span class="cond-mark ' + (ok || i < 2 ? 'yes' : 'no') + '">' + (ok || i < 2 ? '✓' : '–') + '</span>' + c + '</li>').join('') + '</ul>';
    } else if (f.kind === 'cronjobs') {
      props = kv('Created', fmtAge(r.age) + ' ago') + kv('Namespace', nsLink(r.ns)) + kv('Schedule', '<span class="mono">' + esc(r.schedule) + '</span>') + kv('Last run', fmtAge(r.last) + ' ago') + kv('Status', r.suspend ? pill('Suspended', 'warn') : pill('Active', 'success'));
    } else if (OWNER_LABEL[f.kind]) {
      const st = f.kind === 'jobs' ? [r.state, r.state === 'Complete' ? 'success' : r.state === 'Failed' ? 'danger' : 'warn'] : workloadState(r);
      props = kv('Created', fmtAge(r.age) + ' ago') + kv('Namespace', nsLink(r.ns)) + kv(f.kind === 'jobs' ? 'Completions' : 'Replicas', r.have + ' ready / ' + r.want + ' desired') + kv('Status', pill(st[0], st[1]));
      const ps = podsOf(f.kind, r);
      extra = '<div class="inspector-eyebrow">Pods</div>' + (ps.length ? '<ul class="pod-links">' + ps.map(p =>
        '<li><button type="button" class="pod-link" data-pod="' + esc(p.id) + '"><span class="dot ' + toneDot(podTone(p.status)) + '"></span><span class="mono">' + esc(p.name) + '</span></button></li>').join('') + '</ul>' : '<p class="muted-note">No pods currently managed by this resource.</p>');
    } else {
      props = kv('Created', fmtAge(r.age) + ' ago') + (K.cluster ? '' : kv('Namespace', nsLink(r.ns)));
      K.cols.forEach(c => {
        if (c.id === 'name' || c.id === 'ns' || c.id === 'age') return;
        const col = G[f.kind] && G[f.kind].cols.find(x => x[0] === c.id);
        props += kv(esc(c.label), col && col[2] === 'pill' ? pill(String(r[c.id]), tone(r[c.id])) : '<span class="mono">' + esc(r[c.id] == null ? '—' : r[c.id]) + '</span>');
      });
    }
    if (f.kind === 'helm') props += kv('Deployed resources', '<button type="button" class="link-btn" data-tab-go="resources">' + r.resources + ' resources →</button>');
    return '<dl class="kv-list">' + props + '</dl><div class="inspector-eyebrow">Labels</div>' + chipsOf(labelsFor(f.kind, r)) +
      '<div class="inspector-eyebrow">Annotations</div><dl class="kv-list">' + kv('kubectl.kubernetes.io/last-applied', '<span class="mono">' + esc(iso(r.age || 0)) + '</span>') + '</dl>' + schedHtml(f.kind, r) + extra;
  }
  function containersPanel(f){
    const r = f.r;
    return r.ctrs.map((c, i) => {
      const tn = podTone(r.status);
      return '<div class="ctr-card"><div class="ctr-card-head"><span class="dot ' + toneDot(tn) + '"></span><span class="ctr-name mono">' + esc(c) + '</span>' + pill(r.status, tn) + '</div><dl class="kv-list">' +
        kv('Image', '<span class="mono">' + esc(imageOf(c)) + '</span>') + kv('Ports', '<span class="mono">' + (i ? '15001' : '8080') + '/TCP</span>') + kv('Restarts', r.restarts) +
        kv('Requests', '<span class="mono">100m · 128Mi</span>') + kv('Limits', '<span class="mono">500m · 512Mi</span>') +
        kv('Liveness', '<span class="mono">GET :' + (i ? '15021' : '8080') + '/healthz</span>') + kv('Mounts', '2 volumes') + '</dl>' +
        '<div class="ctr-card-actions"><button type="button" class="btn btn-secondary" data-ctr="' + esc(c) + '" data-do="logs">' + ICON('logs') + 'Logs</button><button type="button" class="btn btn-secondary" data-ctr="' + esc(c) + '" data-do="shell">' + ICON('terminal') + 'Shell</button></div></div>';
    }).join('');
  }
  /* Primary actions stay in the row under the title; less-used and destructive ones are tucked into the header's ⋮ menu. */
  const TUCKED = new Set(['scale', 'restart', 'cordon', 'drain', 'delete']);
  let inspMenuItems = [];
  function setInspMenu(items){ inspMenuItems = items; $('#inspMore').hidden = !items.length; }
  /* ---- YAML: read-only view, or an editable draft (Edit in the row, the ⋮ menu or the YAML tab) */
  S.edit = null;
  function yamlPanel(f){
    const r = f.r, id = f.r.id, ed = S.edit && S.edit.id === id ? S.edit : null;
    if (!ed) return '<div class="yaml-bar"><span class="muted-note">Read-only manifest</span><span class="yaml-btns"><button type="button" class="btn btn-secondary" data-do="copyyaml">' + ICON('copy') + 'Copy</button><button type="button" class="btn btn-secondary" data-do="editmanifest">' + ICON('pencil') + 'Edit</button></span></div><pre class="yaml" tabindex="0">' + yamlHtml(toYaml(manifest(f.kind, r))) + '</pre>';
    return '<div class="yaml-bar"><span class="muted-note" id="yamlNote"></span><span class="yaml-btns"><button type="button" class="btn btn-secondary" data-do="editcancel">' + ICON('close') + 'Discard changes</button><button type="button" class="btn btn-primary" data-do="editapply" id="yamlApply">' + ICON('check') + 'Apply changes</button></span></div>' +
      '<div class="yaml-ed"><div class="yaml-gutter" id="yamlGutter" aria-hidden="true"></div><div class="yaml-stack"><pre class="yaml-hl" id="yamlHl" aria-hidden="true"></pre>' +
      '<textarea class="yaml-edit" id="yamlEdit" spellcheck="false" autocapitalize="off" autocomplete="off" aria-label="Edit manifest YAML" aria-describedby="yamlStatus">' + esc(ed.text) + '</textarea></div></div>' +
      '<div class="yaml-status" id="yamlStatus" role="status" aria-live="polite"></div><ul class="yaml-issues" id="yamlIssues"></ul>';
  }
  /* re-run the verifier and repaint highlighting, line numbers, status and the Apply button */
  function refreshEditor(){
    const ed = S.edit, f = ed && byId(ed.id), ta = $('#yamlEdit');
    if (!f || !ta) return;
    const issues = yamlLint(ed.text, f.r ? f : null), errs = issues.filter(x => x.sev === 'error'), warns = issues.filter(x => x.sev === 'warn');
    const bad = new Map(); issues.forEach(x => { if (!bad.has(x.line) || x.sev === 'error') bad.set(x.line, x.sev); });
    const lines = ed.text.split('\n');
    $('#yamlHl').innerHTML = lines.map((l, i) => '<span class="yl' + (bad.get(i + 1) ? ' ' + bad.get(i + 1) : '') + '">' + (yamlLineHtml(l) || ' ') + '</span>').join('\n') + '\n';
    $('#yamlGutter').innerHTML = lines.map((l, i) => '<span class="gl' + (bad.get(i + 1) ? ' ' + bad.get(i + 1) : '') + '">' + (i + 1) + '</span>').join('\n') + '\n';
    const st = $('#yamlStatus');
    st.className = 'yaml-status ' + (errs.length ? 'bad' : warns.length ? 'warn' : 'ok');
    st.textContent = errs.length ? errs.length + (errs.length === 1 ? ' error' : ' errors') + (warns.length ? ', ' + warns.length + ' warning' + (warns.length === 1 ? '' : 's') : '') + ' · fix before applying' : warns.length ? 'Valid YAML · ' + warns.length + ' warning' + (warns.length === 1 ? '' : 's') : 'Valid YAML';
    $('#yamlIssues').innerHTML = issues.slice(0, 12).map(x => '<li class="' + x.sev + '" data-yl="' + x.line + '" data-yc="' + x.col + '" tabindex="0"><span class="yl-ln">Line ' + x.line + '</span>' + esc(x.msg) + '</li>').join('') + (issues.length > 12 ? '<li class="more">+' + (issues.length - 12) + ' more</li>' : '');
    const ap = $('#yamlApply'); ap.disabled = errs.length > 0; ap.title = errs.length ? 'Fix the ' + errs.length + ' error' + (errs.length === 1 ? '' : 's') + ' first' : 'Apply changes'; ap.setAttribute('aria-label', ap.title);
    const dirty = ed.text !== ed.orig;
    $('#yamlNote').innerHTML = dirty ? '<span class="tag">Modified</span> Unsaved changes' : 'Editing manifest';
    syncEditorScroll();
  }
  function syncEditorScroll(){ const ta = $('#yamlEdit'); if (!ta) return; const hl = $('#yamlHl'), g = $('#yamlGutter'); hl.scrollTop = ta.scrollTop; hl.scrollLeft = ta.scrollLeft; g.scrollTop = ta.scrollTop; }
  function startEdit(id){
    const f = byId(id);
    if (!f) return;
    if (S.inspect !== id || !inspector.classList.contains('open')) openInspector(id);
    const text = toYaml(manifest(f.kind, f.r));
    if (!S.edit || S.edit.id !== id) S.edit = {id, text, orig:text};
    S.inspTab = 'yaml'; renderInspector(id);
    const t = $('#yamlEdit'); if (t) t.focus();
  }
  function applyEdit(){
    const ed = S.edit, f = ed && byId(ed.id);
    if (!f) return;
    const bad = yamlLint(ed.text, f).filter(x => x.sev === 'error');
    if (bad.length) { refreshEditor(); const li = $('#yamlIssues li'); if (li) li.focus(); return; }
    const rep = ed.text.match(/^\s+replicas:\s*(\d+)/m);
    if (rep && ('want' in f.r)) { const n = Math.min(50, Number(rep[1])); f.r.want = n; f.r.have = n; }
    const changed = ed.text !== ed.orig;
    S.edit = null; renderRows(); renderInspector(ed.id);
    showToast(changed ? 'Applied changes to ' + f.r.name : 'No changes to apply');
  }
  function renderInspector(id){
    if (S.argoView) return renderArgoDetail();
    if (S.metricView) return renderMetricDetail();
    const f = byId(id);
    if (!f) return closeInspector();
    const r = f.r, K = KINDS[f.kind];
    $('#inspectorName').textContent = r.name; $('#copyName').hidden = false;
    $('#inspectorKind').innerHTML = esc(K.one) + (K.cluster ? '' : ' · ' + nsLink(r.ns));
    const order = K.actions.filter(a => !TUCKED.has(a) && a !== 'edit');   // Edit lives in the ⋮ menu
    setInspMenu((K.actions.indexOf('edit') >= 0 ? ['edit'] : []).concat(K.actions.filter(a => TUCKED.has(a)).sort((x, y) => (x === 'delete') - (y === 'delete'))).map(a => ({act:a, label:a === 'edit' ? 'Edit YAML' : ACTION[a].label, icon:ACTION[a].icon, danger:!!ACTION[a].danger})));
    const qb = a => '<button type="button" class="qa' + (a === 'logs' ? ' primary' : '') + (a === 'delete' ? ' danger' : '') + '" data-act="' + a + '">' + ICON(ACTION[a].icon) + ACTION[a].label + '</button>';
    // Logs and Shell sit on the left; everything else on the right (or in the ⋮ menu)
    const qa = '<div class="qa-row split" data-id="' + esc(id) + '"><span class="qa-left">' + order.filter(a => a === 'logs' || a === 'shell').map(qb).join('') + '</span><span class="qa-right">' + order.filter(a => a !== 'logs' && a !== 'shell').map(qb).join('') + '</span></div>';
    const tabsDef = [['overview', 'Overview']].concat(f.kind === 'pods' ? [['containers', 'Containers']] : [], METRIC_KINDS.has(f.kind) ? [['metrics', 'Metrics']] : [], f.kind === 'crds' ? [['resources', 'Resources'], ['schema', 'Schema']] : f.kind === 'helm' ? [['resources', 'Resources']] : [], [['yaml', 'YAML'], ['events', 'Events']]);
    if (!tabsDef.some(t => t[0] === S.inspTab)) S.inspTab = 'overview';
    const bar = '<div class="insp-tabs" role="tablist">' + tabsDef.map(t => '<button type="button" class="insp-tab" role="tab" data-tab="' + t[0] + '" aria-selected="' + (S.inspTab === t[0]) + '">' + t[1] + '</button>').join('') + '</div>';
    let panel;
    if (S.inspTab === 'metrics') panel = metricsPanel(f);
    else if (S.inspTab === 'resources') panel = resourcesPanel(f);
    else if (S.inspTab === 'schema') panel = schemaHtml(SCHEMAS[f.r.name] || [F_('spec', 'object', 'No schema published for this definition', false)], 0);
    else if (S.inspTab === 'containers') panel = containersPanel(f);
    else if (S.inspTab === 'yaml') panel = yamlPanel(f);
    else if (S.inspTab === 'events') panel = '<ul class="ev-list">' + eventsFor(r, f.kind).map(e => '<li class="' + e[0].toLowerCase() + '"><span class="ev-type">' + esc(e[1]) + '</span><span class="ev-msg">' + esc(e[2]) + '</span><span class="ev-age">' + e[3] + '</span></li>').join('') + '</ul>';
    else panel = overviewPanel(f);
    inspectorBody.innerHTML = qa + bar + '<div class="insp-panel" role="tabpanel">' + panel + '</div>';
    if (S.edit && S.inspTab === 'yaml' && S.edit.id === id) refreshEditor();
  }
  function openInspector(id){
    S.metricView = null; S.argoView = null;
    if (S.inspect !== id) S.inspTab = 'overview';
    S.inspect = id;
    renderInspector(id);
    inspector.classList.add('open');
    inspectorScrim.classList.add('show');
    $$('tr.selected', tbody).forEach(t => t.classList.remove('selected'));
    const tr = $('tr[data-id="' + CSS.escape(id) + '"]', tbody); if (tr) tr.classList.add('selected');
  }
  function closeInspector(){
    navClear(); S.edit = null;
    S.inspect = null; S.metricView = null; S.argoView = null;
    inspector.classList.remove('open');
    inspectorScrim.classList.remove('show');
    $$('tr.selected', tbody).forEach(t => t.classList.remove('selected'));
  }
  function gotoResource(id, tab){
    const f = byId(id);
    if (!f) return;
    return stepTo(() => gotoResourceNow(id, tab));
  }
  function gotoResourceNow(id, tab){
    const f = byId(id);
    setView('workloads', f.kind);
    openInspector(id);
    if (tab) { S.inspTab = tab; renderInspector(id); }
  }
  inspectorBody.addEventListener('click', e => {
    if (S.metricView) {
      const g = e.target.closest('[data-goto]');
      if (g && !(e.target.closest('.topn-row') && Date.now() - topGuard < 700)) gotoResource(g.dataset.goto);
      return;
    }
    const id = S.inspect, f = id && byId(id);
    if (!f) return;
    if (e.target.closest('.topn-row') && Date.now() - topGuard < 700) return;
    const qa = e.target.closest('.qa');
    if (qa) { if (qa.dataset.act === 'logs' || qa.dataset.act === 'shell') closeInspector(); runAction(qa.dataset.act, id); return; }
    const dr = e.target.closest('[data-drill]');
    if (dr) { drill(dr.dataset.drill); return; }
    const tg = e.target.closest('[data-tab-go]');
    if (tg) { S.inspTab = tg.dataset.tabGo; renderInspector(id); return; }
    const tab = e.target.closest('.insp-tab');
    if (tab) { S.inspTab = tab.dataset.tab; renderInspector(id); return; }
    const go = e.target.closest('[data-goto]');
    if (go) return gotoResource(go.dataset.goto);
    const pl = e.target.closest('.pod-link');
    if (pl) return gotoResource(pl.dataset.pod);
    const cb = e.target.closest('[data-ctr]');
    if (cb) { closeInspector(); if (cb.dataset.do === 'logs') openLog(f.r, cb.dataset.ctr); else openShell(f.r); return; }
    const dobtn = e.target.closest('[data-do="copyyaml"]');
    if (dobtn) copyText(toYaml(manifest(f.kind, f.r)), 'Manifest copied');
    const dob = e.target.closest('[data-do]');
    if (dob && dob.dataset.do === 'editmanifest') return startEdit(id);
    if (dob && dob.dataset.do === 'editapply') return applyEdit();
    if (dob && dob.dataset.do === 'editcancel') { S.edit = null; renderInspector(id); return; }
  });
  function copyText(text, msg){
    const done = () => showToast(msg);
    if (navigator.clipboard && navigator.clipboard.writeText) navigator.clipboard.writeText(text).then(done, done); else done();
  }
  $('#closeInspector').addEventListener('click', closeInspector);
  inspectorScrim.addEventListener('click', closeInspector);

  /* ------------------------------------------------------------ navigation */
  function setView(viewId, kind){
    navClear();
    S.from = null; S.flt = null;
    views.forEach(v => v.classList.toggle('active', v.id === 'view-' + viewId));
    if (viewId === 'workloads') setKind(kind || S.kind);
    const kindNow = viewId === 'workloads' ? S.kind : null;
    $$('[data-view]').forEach(b => b.classList.toggle('active', b.dataset.view === viewId && (!b.dataset.kind || b.dataset.kind === kindNow)));
    S.view = viewId;
    const isArgo = viewId === 'argocd', isOv = viewId === 'overview';
    $('#crumbCluster').classList.toggle('active', isOv);
    $('#crumbTrail').hidden = false;
    $('#crumbSection').textContent = isOv ? 'Cluster' : isArgo ? 'Argo CD' : KINDS[S.kind].group;
    if (isArgo) $('#crumbCurrent').textContent = 'Applications';
    if (isOv) $('#crumbCurrent').textContent = 'Overview';
    S.navTab = curGroup();
    const here = viewId === 'overview' ? 'Cluster' : isArgo ? 'Argo CD' : KINDS[S.kind].group;
    Array.from(navOpen).forEach(g => { if (NAV_PINNED.indexOf(g) < 0) navOpen.delete(g); });
    navOpen.add(here);
    renderNav();
    renderSubTabs();
    closePops();
    if (viewId === 'overview') renderOverview();
    if (viewId === 'argocd') renderArgo();
    closeSidebar(); closeMenu(); closeRowMenu();
    main.scrollTop = 0;
  }
  document.addEventListener('click', e => { const b = e.target.closest('[data-view]'); if (b) setView(b.dataset.view, b.dataset.kind); });
  $('#crumbCluster').setAttribute('aria-haspopup', 'menu');
  $('#crumbCluster').addEventListener('click', e => { const b = e.currentTarget; if (clusterMenu.hidden) openClusterMenu(b); else closePops(); });

  /* ------------------------------------------------ metrics, tree, overview */
  const C = {cpu:'var(--accent)', mem:'var(--violet)', rx:'var(--success)', tx:'var(--warn)', fr:'var(--brand)', fw:'var(--argo)'};
  const OK = window.OKCharts;
  const NS_KINDS = ['pods', 'deployments', 'statefulsets', 'daemonsets', 'jobs', 'cronjobs', 'services', 'ingresses', 'configmaps', 'secrets', 'pvcs', 'serviceaccounts'];
  const TREE_KINDS = new Set(['pods', 'deployments', 'replicasets', 'statefulsets', 'daemonsets', 'jobs', 'cronjobs', 'nodes', 'namespaces', 'crds', 'helm', 'services']);
  const AUTO_EXPAND = new Set();   // rows start collapsed; use the chevrons or Expand all
  const SUMMARY_KINDS = new Set(['pods', 'deployments', 'statefulsets', 'daemonsets', 'namespaces', 'nodes']);
  const METRIC_KINDS = new Set(['pods', 'deployments', 'statefulsets', 'daemonsets', 'replicasets', 'namespaces', 'nodes']);
  const visited = new Set(['pods']);
  const childReg = new Map();
  S.range = '1h'; S.open = new Set(); S.from = null; S.sumOpen = window.innerWidth >= 768; S.evFilter = 'all';

  function scaleOf(kind, r){
    if (kind === 'pods' || kind === 'jobs') return 1;
    if (kind === 'namespaces') return Math.max(1, DATA.pods.filter(p => p.ns === r.name).length);
    if (kind === 'nodes') return 24;
    if (r.want != null) return Math.max(1, r.want);
    if (r.desired != null) return Math.max(1, r.desired);
    return 1;
  }
  function metricsOf(kind, r, range){
    const o = kind === 'nodes' ? {cpuLimit:8000, cpuReq:4800, memLimit:32768, memReq:20000} : null;
    return OK.metrics(kind + ':' + r.id, scaleOf(kind, r), range || S.range, o);
  }
  function aggMetrics(list, range){
    const sum = f => OK.sumSeries(list.map(f));
    return {range, cpu:{data:sum(m => m.cpu.data), request:list.reduce((a, m) => a + m.cpu.request, 0), limit:list.reduce((a, m) => a + m.cpu.limit, 0)},
      mem:{data:sum(m => m.mem.data), request:list.reduce((a, m) => a + m.mem.request, 0), limit:list.reduce((a, m) => a + m.mem.limit, 0)},
      rx:sum(m => m.rx), tx:sum(m => m.tx), fr:sum(m => m.fr), fw:sum(m => m.fw), fsPct:Math.round(list.reduce((a, m) => a + m.fsPct, 0) / list.length)};
  }
  function chartSet(m, scope){
    const r = m.range, sc = scope || '';
    return OK.chart('CPU', [{name:'usage', color:C.cpu, data:m.cpu.data}], {unit:'cpu', range:r, request:m.cpu.request, limit:m.cpu.limit, metric:'cpu', scope:sc}) +
      OK.chart('Memory', [{name:'working set', color:C.mem, data:m.mem.data}], {unit:'mem', range:r, request:m.mem.request, limit:m.mem.limit, metric:'mem', scope:sc}) +
      OK.chart('Network', [{name:'receive', color:C.rx, data:m.rx}, {name:'transmit', color:C.tx, data:m.tx}], {unit:'bps', range:r, metric:'net', scope:sc}) +
      OK.chart('Filesystem I/O', [{name:'read', color:C.fr, data:m.fr}, {name:'write', color:C.fw, data:m.fw}], {unit:'bps', range:r, metric:'fs', scope:sc});
  }
  const hbar = (pct, cls) => '<span class="hbar"><span class="track"><span class="fill ' + (cls || (pct >= 85 ? 'danger' : pct >= 70 ? 'warn' : '')) + '" style="width:' + Math.min(100, Math.max(2, pct)) + '%"></span></span><span class="num">' + Math.round(pct) + '%</span></span>';
  const lastOf = OK.last;

  /* ---- compact "Top N by metric" ranking, shared by the usage panel and the details panel */
  const TOP_METRICS = [['cpu', 'CPU'], ['mem', 'Memory'], ['rx', 'Network in'], ['tx', 'Network out'], ['fr', 'Disk read'], ['fw', 'Disk write'], ['restarts', 'Restarts']];
  const TOP_VIEWS = [['top-3', 'Top 3'], ['top-5', 'Top 5'], ['top-10', 'Top 10'], ['bottom-5', 'Lowest 5']];
  try { S.top = Object.assign({metric:'cpu', view:'top-5'}, JSON.parse(lsGet('openkite-top', '{}'))); } catch (err) { S.top = {metric:'cpu', view:'top-5'}; }
  if (!TOP_METRICS.some(x => x[0] === S.top.metric)) S.top.metric = 'cpu';
  if (!TOP_VIEWS.some(x => x[0] === S.top.view)) S.top.view = 'top-5';
  function restartsOf(kind, r){ return kind === 'pods' ? r.restarts : podsUnder(kind, r).reduce((a, p) => a + p.restarts, 0); }
  function topValue(metric, e){
    const F = OK.FMT, m = e.m;
    if (metric === 'restarts') { const v = restartsOf(e.kind, e.r); return {v, text:v + '×', pct:null}; }
    if (metric === 'cpu' || metric === 'mem') { const d = m[metric], v = lastOf(d.data); return {v, text:F[metric](v), pct:v / d.limit * 100}; }
    const v = lastOf(m[metric]); return {v, text:F.bps(v), pct:null};
  }
  let topGuard = 0;
  function topPanel(entries, src, popts){
    const list = popts && popts.metrics ? TOP_METRICS.filter(x => popts.metrics.indexOf(x[0]) >= 0) : TOP_METRICS;
    const metric = list.some(x => x[0] === S.top.metric) ? S.top.metric : list[0][0], [dir, nStr] = S.top.view.split('-'), n = Number(nStr), name = (TOP_METRICS.find(x => x[0] === metric) || [])[1];
    const scored = entries.map(e => Object.assign({e}, topValue(metric, e)));
    scored.sort((a, b) => dir === 'top' ? b.v - a.v : a.v - b.v);
    const shown = scored.slice(0, n), mx = Math.max.apply(null, shown.map(x => x.v).concat([1e-9]));
    const total = scored.reduce((a, x) => a + x.v, 0), F = OK.FMT;
    const totalText = metric === 'restarts' ? total + '×' : metric === 'cpu' ? F.cpu(total) : metric === 'mem' ? F.mem(total) : F.bps(total);
    const opts = (list, cur) => list.map(x => '<option value="' + x[0] + '"' + (x[0] === cur ? ' selected' : '') + '>' + esc(x[1]) + '</option>').join('');
    const attr = src === 'summary' ? 'data-open' : 'data-goto';
    const head = '<div class="topn-head"><span class="topn-title"><select data-topn="view" aria-label="How many to show">' + opts(TOP_VIEWS, S.top.view) + '</select><span>by</span><select data-topn="metric" aria-label="Rank by">' + opts(list, metric) + '</select></span>' +
      '<span class="topn-sub" title="Combined ' + esc(String(name)) + ' of all ' + entries.length + '">' + cnt(entries.length) + ' total · Σ ' + esc(totalText) + '</span></div>';
    const rows = shown.map((x, i) => {
      const tone = x.pct == null ? '' : x.pct >= 85 ? ' danger' : x.pct >= 70 ? ' warn' : '';
      return '<li><div class="topn-row" role="button" tabindex="0" ' + attr + '="' + esc(x.e.id) + '" title="' + esc(x.e.name + (x.e.sub ? ' · ' + x.e.sub : '')) + '"><span class="topn-rank">' + (i + 1) + '</span><span class="topn-name">' + esc(x.e.name) + (x.e.sub ? '<small>' + esc(x.e.sub) + '</small>' : '') + '</span>' +
        '<span class="topn-bar' + tone + '"><i style="width:' + Math.max(3, x.v / mx * 100) + '%"></i></span><span class="topn-val">' + esc(x.text) + '</span><span class="topn-pct' + tone + '">' + (x.pct == null ? '' : Math.round(x.pct) + '%') + '</span></div></li>';
    }).join('');
    const legend = metric === 'cpu' || metric === 'mem' ? '<div class="topn-note">% is the share of the configured limit</div>' : '';
    return '<div class="topn" data-topn-src="' + src + '">' + head + '<ol class="topn-list">' + rows + '</ol>' + legend + '</div>';
  }
  const isSelect = t => !!(t && t.closest && t.closest('select'));
  document.addEventListener('pointerdown', e => { if (isSelect(e.target)) topGuard = Date.now(); }, true);
  document.addEventListener('change', e => { if (e.target && e.target.tagName === 'SELECT') topGuard = Date.now(); }, true);
  document.addEventListener('focusout', e => { if (e.target && e.target.tagName === 'SELECT') topGuard = Date.now(); }, true);
  document.addEventListener('click', e => {
    // The click that closes a native dropdown (or the one right after choosing an option) can land on a table row,
    // a graph or a button that sits under the open list. Swallow anything that isn't the dropdown itself.
    if (Date.now() - topGuard < 700 && !isSelect(e.target)) { e.stopPropagation(); e.preventDefault(); }
  }, true);
  document.addEventListener('change', e => {
    const sel = e.target.closest && e.target.closest('select[data-topn]');
    if (!sel) return;
    topGuard = Date.now();   // the click that closes a native dropdown can land on a row after the re-render; ignore it
    S.top[sel.dataset.topn] = sel.value;
    lsSet('openkite-top', JSON.stringify(S.top));
    const src = sel.closest('.topn').dataset.topnSrc;
    if (src === 'summary') renderSummary();
    else if (src === 'metric') { const sc = inspectorBody.scrollTop; renderMetricDetail(); inspectorBody.scrollTop = sc; }
    else if (S.inspect) { const sc = inspectorBody.scrollTop; renderInspector(S.inspect); inspectorBody.scrollTop = sc; }
    const again = $((src === 'summary' ? '#wlSummary' : '#inspectorBody') + ' select[data-topn="' + sel.dataset.topn + '"]'); if (again) again.focus();
  });

  /* ---- metric detail pane: opens when a chart is clicked */
  const MD = {
    cpu:{title:'CPU', unit:'cpu', top:['cpu'], series:m => [{name:'usage', color:C.cpu, data:m.cpu.data}], limits:m => m.cpu, blurb:'CPU time consumed, summed over the pods in scope. Sustained use near the limit means throttling; far below the request means the request is oversized.'},
    mem:{title:'Memory', unit:'mem', top:['mem'], series:m => [{name:'working set', color:C.mem, data:m.mem.data}], limits:m => m.mem, blurb:'Working-set memory. Crossing the limit gets a container OOM-killed, so keep an eye on the peak, not only the average.'},
    net:{title:'Network', unit:'bps', top:['rx', 'tx'], series:m => [{name:'receive', color:C.rx, data:m.rx}, {name:'transmit', color:C.tx, data:m.tx}], limits:null, blurb:'Traffic in and out of the pods in scope. Sudden asymmetry between receive and transmit often points at a retry storm or a chatty dependency.'},
    fs:{title:'Filesystem I/O', unit:'bps', top:['fr', 'fw'], series:m => [{name:'read', color:C.fr, data:m.fr}, {name:'write', color:C.fw, data:m.fw}], limits:null, blurb:'Disk throughput on the volumes in scope. High, steady writes with a growing mount usually means logs or a cache that is not being pruned.'}
  };
  function scopeInfo(scope){
    if (scope === 'cluster') {
      const ps = NSD('pods'), ms = ps.map(p => metricsOf('pods', p));
      return ps.length ? {label:'Cluster', m:aggMetrics(ms, S.range), entries:ps.map((p, i) => ({id:p.id, name:p.name, sub:p.ns, kind:'pods', r:p, m:ms[i]}))} : null;
    }
    if (scope.indexOf('sum:') === 0) {
      const kind = scope.slice(4), rows = kind === S.kind ? visibleRows() : [];
      if (!rows.length) return null;
      const ms = rows.map(r => metricsOf(kind, r));
      return {label:KINDS[kind].label + ' (' + rows.length + ')', m:aggMetrics(ms, S.range), entries:rows.map((r, i) => ({id:r.id, name:r.name, sub:r.ns || '', kind, r, m:ms[i]}))};
    }
    const f = byId(scope.slice(4));
    if (!f) return null;
    const kids = podsUnder(f.kind, f.r);
    return {label:f.r.name, m:metricsOf(f.kind, f.r), entries:kids.map(p => ({id:p.id, name:p.name, sub:p.ns, kind:'pods', r:p, m:metricsOf('pods', p)})), back:f};
  }
  function openMetricDetail(metric, scope){
    if (!MD[metric]) return;
    S.argoView = null;
    S.metricView = {metric, scope, back:S.inspect};
    S.inspect = null;
    $$('tr.selected', tbody).forEach(t => t.classList.remove('selected'));
    renderMetricDetail();
    inspector.classList.add('open');
    inspectorScrim.classList.add('show');
    inspectorBody.scrollTop = 0;
  }
  function renderMetricDetail(){
    const mv = S.metricView;
    if (!mv) return;
    const def = MD[mv.metric], info = scopeInfo(mv.scope);
    $('#copyName').hidden = true; setInspMenu([]);
    $('#inspectorName').textContent = def.title + (info ? ' · ' + info.label : '');
    $('#inspectorKind').textContent = 'Metric details · last ' + S.range;
    if (!info) { inspectorBody.innerHTML = '<p class="muted-note">There is no data for this selection any more.</p>'; return; }
    const m = info.m, ser = def.series(m), F = OK.FMT, fmt = F[def.unit], lim = def.limits ? def.limits(m) : null;
    const back = mv.back && byId(mv.back) ? '<button type="button" class="link-back" data-mback="' + esc(mv.back) + '">← Back to ' + esc(byId(mv.back).r.name) + '</button>' : '';
    const rows = ser.map(s => { const st = OK.stats(s.data, S.range); return '<tr><th scope="row"><i style="background:' + s.color + '"></i>' + esc(s.name) + '</th><td>' + esc(fmt(st.now)) + '</td><td>' + esc(fmt(st.avg)) + '</td><td>' + esc(fmt(st.p95)) + '</td><td>' + esc(fmt(st.max)) + '<small>' + esc(st.maxAt) + '</small></td><td>' + esc(fmt(st.min)) + '</td></tr>'; }).join('');
    let limits = '';
    if (lim) {
      const d = ser[0].data, now = lastOf(d), st = OK.stats(d, S.range), pctOf = v => Math.round(v / lim.limit * 100), reqPct = v => Math.round(v / lim.request * 100);
      const above = Math.round(d.filter(v => v > lim.request).length / d.length * 100);
      // Judge the whole window, not just the latest sample: p95 is "typical busy", peak is the worst case.
      let verdict, advice;
      if (st.max >= lim.limit * 0.9 || st.p95 >= lim.limit * 0.8) {
        verdict = ['danger', 'Underprovisioned'];
        advice = (mv.metric === 'cpu' ? 'CPU is likely throttled at peak. ' : 'Memory is at risk of OOM-kill at peak. ') + 'Raise the limit to about ' + fmt(st.max * 1.3) + ' and the request to about ' + fmt(st.p95 * 1.1) + '.';
      } else if (st.p95 < lim.request * 0.5 || st.max < lim.limit * 0.35) {
        verdict = ['info', 'Overprovisioned'];
        advice = 'Reserved capacity is mostly idle. Lower the request to about ' + fmt(Math.max(st.p95 * 1.2, lim.limit * 0.05)) + (st.max < lim.limit * 0.35 ? ' and the limit to about ' + fmt(st.max * 2) : '') + ' to free room on the node.';
      } else {
        verdict = ['ok', 'Healthy'];
        advice = 'Usage sits comfortably between the request and the limit. No change needed.';
      }
      limits = '<div class="inspector-eyebrow">Against requests and limits</div><dl class="kv-list">' + kv('Request', '<span class="mono">' + esc(fmt(lim.request)) + '</span>') + kv('Limit', '<span class="mono">' + esc(fmt(lim.limit)) + '</span>') +
        kv('Typical (avg · p95)', '<span class="mono">' + esc(fmt(st.avg)) + ' · ' + esc(fmt(st.p95)) + '</span> <small class="muted">' + reqPct(st.p95) + '% of request</small>') +
        kv('Peak', '<span class="mono">' + esc(fmt(st.max)) + '</span> <small class="muted">' + pctOf(st.max) + '% of limit</small>') +
        kv('Headroom to limit', '<span class="mono">' + esc(fmt(Math.max(0, lim.limit - now))) + ' (' + Math.max(0, 100 - pctOf(now)) + '%)</span>') + kv('Above request', above + '% of the window') +
        kv('Assessment', '<span class="pill ' + (verdict[0] === 'danger' ? 'danger' : verdict[0] === 'info' ? 'info' : 'success') + '">' + esc(verdict[1]) + '</span>') + '</dl>' +
        '<p class="assess-note ' + verdict[0] + '">' + esc(advice) + '</p>';
    }
    const chart = OK.chart(def.title, ser, {unit:def.unit, range:S.range, h:176, request:lim && lim.request, limit:lim && lim.limit});
    const breakdown = info.entries.length > 1 ? '<div class="inspector-eyebrow">Biggest contributors</div>' + topPanel(info.entries, 'metric', {metrics:def.top}) : '';
    inspectorBody.innerHTML = back + '<div class="yaml-bar">' + OK.rangeTabs(S.range) + '</div>' + '<div class="chart-grid one">' + chart + '</div>' +
      '<div class="inspector-eyebrow">Over the last ' + S.range + '</div><div class="table-scroll"><table class="mstats"><thead><tr><th></th><th>Now</th><th>Avg</th><th>p95</th><th>Peak</th><th>Min</th></tr></thead><tbody>' + rows + '</tbody></table></div>' +
      limits + breakdown + '<p class="muted-note mblurb">' + esc(def.blurb) + '</p>';
  }
  document.addEventListener('click', e => {
    const card = e.target.closest('.chart-card[data-metric]');
    if (card && Date.now() - topGuard < 700) return;
    if (card && !e.target.closest('select,.range-tabs,button,a') && !(S.metricView && inspector.contains(card))) { openMetricDetail(card.dataset.metric, card.dataset.scope); return; }
    const back = e.target.closest('[data-mback]');
    if (back && inspector.contains(back)) { const id = back.dataset.mback; S.metricView = null; openInspector(id); S.inspTab = 'metrics'; renderInspector(id); }
  });

  /* ---- usage summary above tables */
  function renderSummary(){
    const el = $('#wlSummary');
    if (!SUMMARY_KINDS.has(S.kind)) { el.hidden = true; return; }
    const rows = visibleRows();
    el.hidden = false;
    el.dataset.open = String(S.sumOpen);
    if (!rows.length) { el.innerHTML = '<div class="panel-head"><h3>Usage</h3></div>'; return; }
    const ms = rows.map(r => metricsOf(S.kind, r));
    const agg = aggMetrics(ms, S.range), F = OK.FMT;
    const cpu = lastOf(agg.cpu.data), mem = lastOf(agg.mem.data), net = lastOf(agg.rx) + lastOf(agg.tx);
    const circles = '<div class="sum-circles"><div class="gauge-row">' + OK.gauge(cpu / agg.cpu.limit * 100, 'CPU', F.cpu(cpu), C.cpu) + OK.gauge(mem / agg.mem.limit * 100, 'Memory', F.mem(mem), C.mem) +
      OK.gauge(net / (300 * rows.length) * 100, 'Network', F.bps(net), C.rx) + OK.gauge(agg.fsPct, 'Filesystem', agg.fsPct + '% used', C.fr) + '</div></div>';
    el.innerHTML = '<div class="panel-head"><button type="button" class="sum-toggle" data-sum aria-expanded="' + S.sumOpen + '">' + ICON('chevron') + 'Usage ' + cnt(rows.length) + esc(KINDS[S.kind].label.toLowerCase()) + '</button>' + OK.rangeTabs(S.range) + '</div>' +
      '<div class="panel-body">' + circles + '<div class="sum-graphs"><div class="chart-grid">' + chartSet(agg, 'sum:' + S.kind) + '</div>' +
      (rows.length > 1 ? topPanel(rows.map((r, i) => ({id:r.id, name:r.name, sub:r.ns || '', kind:S.kind, r, m:ms[i]})), 'summary') : '') + '</div></div>';
  }
  $('#wlSummary').addEventListener('click', e => {
    if (e.target.closest('[data-sum]')) { S.sumOpen = !S.sumOpen; $('#wlSummary').dataset.open = String(S.sumOpen); e.target.closest('[data-sum]').setAttribute('aria-expanded', String(S.sumOpen)); return; }
    const o = e.target.closest('[data-open]:not(#wlSummary)');
    if (o && !(o.closest('.topn-row') && Date.now() - topGuard < 700)) openInspector(o.dataset.open);
  });
  $('#wlSummary').addEventListener('keydown', e => {
    const r = e.target.closest && e.target.closest('.topn-row[data-open]');
    if (r && e.target === r && (e.key === 'Enter' || e.key === ' ')) { e.preventDefault(); openInspector(r.dataset.open); }
  });
  document.addEventListener('click', e => {
    const b = e.target.closest('[data-range]');
    if (!b) return;
    S.range = b.dataset.range;
    renderSummary();
    if (S.metricView && inspector.classList.contains('open')) { const sc = inspectorBody.scrollTop; renderMetricDetail(); inspectorBody.scrollTop = sc; }
    else if (S.inspect && inspector.classList.contains('open')) { const sc = inspectorBody.scrollTop; renderInspector(S.inspect); inspectorBody.scrollTop = sc; }
    if (S.view === 'overview') renderOverview();
  });

  /* ---- hierarchy */
  function statusOf(kind, r){
    if (kind === 'pods') return [r.status, podTone(r.status)];
    if (kind === 'jobs') return [r.state, r.state === 'Complete' ? 'success' : r.state === 'Failed' ? 'danger' : 'warn'];
    if (kind === 'cronjobs') return r.suspend ? ['Suspended', 'warn'] : ['Active', 'success'];
    if (kind === 'replicasets') return [r.ready + '/' + r.desired, r.ready >= r.desired ? 'success' : 'warn'];
    if (r.want != null && r.have != null) return workloadState(r);
    if (r.status != null) return [String(r.status), tone(r.status)];
    if (r.ready != null) return [String(r.ready), tone(r.ready)];
    return null;
  }
  function resNode(kind, r){
    const meta = kind === 'pods' ? r.node : r.schedule || (r.group ? r.group : '');
    return {key:r.id, icon:KINDS[kind].icon, kind:KINDS[kind].one, name:r.name, meta, pill:statusOf(kind, r), go:() => gotoResource(r.id), kids:() => kidsOf(kind, r)};
  }
  function kidsOf(kind, r){
    const pods = fn => DATA.pods.filter(fn).map(p => resNode('pods', p));
    switch (kind) {
      case 'pods': return r.ctrs.map(c => ({key:'c:' + c, icon:'terminal', kind:'Container', name:c, meta:imageOf(c), pill:[r.status, podTone(r.status)], go:() => gotoResource(r.id, 'containers'), kids:() => []}));
      case 'deployments': { const rs = DATA.replicasets.filter(x => x.ns === r.ns && x.name.indexOf(r.name + '-') === 0); return rs.length ? rs.map(x => resNode('replicasets', x)) : podsOf(kind, r).map(p => resNode('pods', p)); }
      case 'replicasets': return pods(p => p.ns === r.ns && p.name.indexOf(r.name + '-') === 0);
      case 'statefulsets': case 'daemonsets': case 'jobs': return podsOf(kind, r).map(p => resNode('pods', p));
      case 'services': return pods(p => p.ns === r.ns && p.name.indexOf(r.name) === 0);
      case 'cronjobs': return DATA.jobs.filter(j => j.ns === r.ns && j.name.indexOf(r.name + '-') === 0).map(j => resNode('jobs', j));
      case 'nodes': return pods(p => p.node === r.name);
      case 'namespaces': return NS_KINDS.map(k => ({k, n:DATA[k].filter(x => x.ns === r.name).length})).filter(x => x.n).map(x =>
        ({key:'k:' + x.k, icon:KINDS[x.k].icon, kind:'', name:x.n + ' ' + (x.n === 1 ? KINDS[x.k].one.toLowerCase() : KINDS[x.k].label.toLowerCase()), meta:'', pill:null, go:() => openKindInNs(x.k, r.name), kids:() => []}));
      case 'crds': { const k = CRD_KIND[r.name]; return k ? DATA[k].map(x => resNode(k, x)) : []; }
      case 'helm': {
        const ORDER = ['deployments', 'statefulsets', 'daemonsets', 'jobs', 'cronjobs', 'services', 'ingresses', 'configmaps', 'secrets', 'hpas', 'pvcs', 'serviceaccounts', 'roles', 'rolebindings', 'servicemonitors', 'networkpolicies', 'certificates', 'crds', 'clusterissuers'];
        const all = helmResources(r).sort((x, y) => ORDER.indexOf(x.kind) - ORDER.indexOf(y.kind)), h = helmHealth(r), CAP = 6;
        const summary = {key:'sum', icon:'check', orb:resOrb(r, null), kind:'', name:h.n + ' resources', meta:(h.ok + ' healthy') + (h.warn ? ' · ' + h.warn + ' progressing' : '') + (h.bad ? ' · ' + h.bad + ' failing' : ''), pill:null, go:() => gotoResource(r.id, 'resources'), kids:() => []};
        const nodes = all.slice(0, CAP).map(x => resNode(x.kind, x.r));
        if (all.length > CAP) nodes.push({key:'more', icon:'arrow', kind:'', name:'Show all ' + all.length + ' in details', meta:(all.length - CAP) + ' more', pill:null, go:() => gotoResource(r.id, 'resources'), kids:() => []});
        return [summary].concat(nodes);
      }
      default: return [];
    }
  }
  function treeRows(nodes, depth, parentKey){
    return nodes.map(n => {
      const key = parentKey + '>' + n.key, kids = n.kids(), open = kids.length && S.open.has(key);
      childReg.set(key, n);
      return '<tr class="child" tabindex="0" data-key="' + esc(key) + '"><td colspan="40"><div class="child-row" style="--d:' + depth + '">' +
        '<span class="guides"><i></i></span>' + (kids.length ? '<button type="button" class="tree-toggle" data-tkey="' + esc(key) + '" aria-expanded="' + !!open + '" aria-label="Toggle ' + esc(n.name) + '">' + ICON('chevron') + '</button>' : '<span class="tree-spacer"></span>') +
        (n.orb || ICON(n.icon)) + (n.kind ? '<span class="child-kind">' + esc(n.kind) + '</span>' : '') + '<span class="child-name">' + esc(n.name) + '</span>' +
        (n.meta ? '<span class="child-meta">' + esc(n.meta) + '</span>' : '<span class="child-meta"></span>') + (n.pill ? pill(n.pill[0], n.pill[1]) : '') + '</div></td></tr>' +
        (open ? treeRows(kids, depth + 1, key) : '');
    }).join('');
  }
  function openAll(key, kids, depth){
    if (depth > 3) return;
    S.open.add(key);
    kids.forEach(n => { const k = key + '>' + n.key, nk = n.kids(); if (nk.length) openAll(k, nk, depth + 1); });
  }
  function setSumOpen(v){
    S.sumOpen = v;
    const el = $('#wlSummary'), t = $('[data-sum]', el);
    el.dataset.open = String(v);
    if (t) t.setAttribute('aria-expanded', String(v));
  }
  function expandAll(){ visibleRows().forEach(r => { const k = kidsOf(S.kind, r); if (k.length) openAll(r.id, k, 1); }); setSumOpen(true); renderRows(); }
  $('#expandAll').addEventListener('click', expandAll);
  $('#collapseAll').addEventListener('click', () => { S.open.clear(); setSumOpen(false); renderRows(); });
  function toggleKey(key){ if (S.open.has(key)) S.open.delete(key); else S.open.add(key); renderRows(); const t = $('[data-tkey="' + CSS.escape(key) + '"],[data-ptoggle="' + CSS.escape(key) + '"]', tbody); if (t) t.focus(); }
  tbody.addEventListener('click', e => {
    const t = e.target.closest('[data-tkey],[data-ptoggle]');
    if (t) { e.stopPropagation(); toggleKey(t.dataset.tkey || t.dataset.ptoggle); return; }
    const hr = e.target.closest('[data-hres]');
    if (hr) { e.stopPropagation(); gotoResource(hr.dataset.hres, 'resources'); return; }
    const d = e.target.closest('[data-drill]');
    if (d) { e.stopPropagation(); drill(d.dataset.drill); return; }
    const tr = e.target.closest('tr.child');
    if (tr) { const n = childReg.get(tr.dataset.key); if (n) n.go(); }
  }, true);
  tbody.addEventListener('keydown', e => {
    const tr = e.target.closest('tr.child');
    if (tr && e.target === tr && (e.key === 'Enter' || e.key === ' ')) { e.preventDefault(); const n = childReg.get(tr.dataset.key); if (n) n.go(); }
  });
  function openKindInNs(kind, ns){ setView('workloads', kind); S.ns = new Set([ns]); commitNs(); }
  function drill(crdName){
    const kind = CRD_KIND[crdName];
    if (!kind) return showToast('No instances are served for ' + crdName);
    stepTo(() => { setView('workloads', kind); S.from = {kind:'crds', name:crdName}; renderWorkloads(); });
  }
  $('#wlBack').addEventListener('click', () => { if (NAV_STACK.length) return navBack(); const n = S.from && S.from.name; setView('workloads', 'crds'); if (n) { const r = DATA.crds.find(x => x.name === n); if (r) openInspector(r.id); } });

  /* ---- inspector: metrics + CRD panels */
  const crdKind = name => { const p = name.split('.')[0]; return p.charAt(0).toUpperCase() + p.slice(1).replace(/ies$/, 'y').replace(/s$/, ''); };
  function podsUnder(kind, r){
    if (kind === 'namespaces') return DATA.pods.filter(p => p.ns === r.name);
    if (kind === 'nodes') return DATA.pods.filter(p => p.node === r.name);
    if (kind === 'replicasets') return DATA.pods.filter(p => p.ns === r.ns && p.name.indexOf(r.name + '-') === 0);
    if (kind === 'pods') return [];
    return podsOf(kind, r);
  }
  function mountsFor(kind, r){
    const g = OK.rng(r.id + 'mnt'), pc = () => Math.round(8 + g() * 62);
    if (kind === 'nodes') return [['/ (root)', pc(), '100 GiB'], ['/var/lib/containerd', pc(), '200 GiB'], ['/var/lib/kubelet', pc(), '50 GiB']];
    if (kind === 'namespaces') { const v = DATA.pvcs.filter(p => p.ns === r.name); return v.length ? v.map(p => [p.name, pc(), p.capacity]) : [['No persistent volumes', 0, '—']]; }
    return [['/ (overlay)', pc(), '20 GiB'], ['/tmp (emptyDir)', Math.round(g() * 20), '1 GiB'], ['/var/log', pc(), '5 GiB'], ['/etc/config (configMap)', 1, '4 KiB']];
  }
  function metricsPanel(f){
    const r = f.r, kind = f.kind, m = metricsOf(kind, r), F = OK.FMT, up = kind !== 'pods' || r.status === 'Running';
    const cpu = lastOf(m.cpu.data), mem = lastOf(m.mem.data), net = lastOf(m.rx) + lastOf(m.tx), sc = scaleOf(kind, r);
    const gauges = '<div class="gauge-row">' + OK.gauge(up ? cpu / m.cpu.limit * 100 : 0, 'CPU', F.cpu(cpu) + ' / ' + F.cpu(m.cpu.limit), C.cpu) + OK.gauge(up ? mem / m.mem.limit * 100 : 0, 'Memory', F.mem(mem) + ' / ' + F.mem(m.mem.limit), C.mem) +
      OK.gauge(up ? net / (300 * sc) * 100 : 0, 'Network', F.bps(net), C.rx) + OK.gauge(m.fsPct, 'Filesystem', m.fsPct + '% used', C.fr) + '</div>';
    const mounts = '<div class="inspector-eyebrow">Filesystem</div><div class="fs-list">' + mountsFor(kind, r).map(x => '<div class="meter"><div class="meter-head"><span class="mono">' + esc(x[0]) + '</span><span class="mono">' + x[1] + '% of ' + esc(x[2]) + '</span></div><div class="meter-track"><span class="meter-fill" style="width:' + Math.max(1, x[1]) + '%"></span></div></div>').join('') + '</div>';
    const netKv = '<div class="inspector-eyebrow">Network</div><dl class="kv-list">' + kv('Receive', F.bps(lastOf(m.rx))) + kv('Transmit', F.bps(lastOf(m.tx))) + (kind === 'pods' ? kv('Interface', '<span class="mono">eth0 · ' + esc(r.ip) + '</span>') : '') + kv('Dropped packets', '0') + '</dl>';
    const kids = podsUnder(kind, r);
    const rank = kids.length > 1 ? '<div class="inspector-eyebrow">Pods ranked</div>' + topPanel(kids.map(p => ({id:p.id, name:p.name, sub:p.ns, kind:'pods', r:p, m:metricsOf('pods', p)})), 'inspector') : '';
    return '<div class="yaml-bar">' + OK.rangeTabs(S.range) + '</div>' + gauges + '<div class="chart-grid compact" style="margin-top:12px">' + chartSet(m, 'res:' + r.id) + '</div>' + mounts + netKv + rank;
  }
  const F_ = (name, type, desc, req, kids) => ({name, type, desc, req, kids});
  const SCHEMAS = {
    'certificates.cert-manager.io':[F_('spec', 'object', 'Desired state of the certificate', true, [F_('secretName', 'string', 'Secret that will hold the key pair', true), F_('issuerRef', 'object', 'Issuer that signs the request', true, [F_('name', 'string', '', true), F_('kind', 'string', 'Issuer or ClusterIssuer'), F_('group', 'string', '')]), F_('dnsNames', '[]string', 'DNS subject alternative names'), F_('duration', 'duration', 'Requested lifetime, for example 2160h'), F_('renewBefore', 'duration', 'How long before expiry to renew'), F_('privateKey', 'object', '', false, [F_('algorithm', 'enum', 'RSA, ECDSA or Ed25519'), F_('size', 'integer', '')])]), F_('status', 'object', 'Observed state', false, [F_('conditions', '[]object', 'Readiness conditions', false, [F_('type', 'string'), F_('status', 'string'), F_('reason', 'string')]), F_('notAfter', 'date-time', 'Expiry of the current certificate'), F_('renewalTime', 'date-time', '')])],
    'clusterissuers.cert-manager.io':[F_('spec', 'object', '', true, [F_('acme', 'object', 'ACME issuer settings', false, [F_('server', 'string', 'ACME directory URL', true), F_('email', 'string', ''), F_('privateKeySecretRef', 'object', '', true, [F_('name', 'string', '', true)]), F_('solvers', '[]object', 'Challenge solvers', false, [F_('http01', 'object', ''), F_('dns01', 'object', '')])]), F_('ca', 'object', 'CA issuer settings', false, [F_('secretName', 'string', '', true)])]), F_('status', 'object', '', false, [F_('conditions', '[]object', '', false, [F_('type', 'string'), F_('status', 'string')])])],
    'applications.argoproj.io':[F_('spec', 'object', '', true, [F_('project', 'string', 'AppProject this application belongs to', true), F_('source', 'object', 'Where manifests come from', true, [F_('repoURL', 'string', '', true), F_('targetRevision', 'string', 'Branch, tag or commit'), F_('path', 'string', 'Directory inside the repository'), F_('chart', 'string', 'Helm chart name')]), F_('destination', 'object', 'Where to deploy', true, [F_('server', 'string'), F_('namespace', 'string')]), F_('syncPolicy', 'object', '', false, [F_('automated', 'object', '', false, [F_('prune', 'boolean'), F_('selfHeal', 'boolean')])])]), F_('status', 'object', '', false, [F_('sync', 'object', '', false, [F_('status', 'string')]), F_('health', 'object', '', false, [F_('status', 'string')])])],
    'appprojects.argoproj.io':[F_('spec', 'object', '', true, [F_('description', 'string'), F_('sourceRepos', '[]string', 'Repositories applications may pull from'), F_('destinations', '[]object', 'Allowed targets', false, [F_('server', 'string'), F_('namespace', 'string')]), F_('clusterResourceWhitelist', '[]object', '', false, [F_('group', 'string'), F_('kind', 'string')])])],
    'servicemonitors.monitoring.coreos.com':[F_('spec', 'object', '', true, [F_('selector', 'object', 'Services to scrape', true, [F_('matchLabels', 'map[string]string')]), F_('namespaceSelector', 'object', '', false, [F_('matchNames', '[]string')]), F_('endpoints', '[]object', 'Ports to scrape', true, [F_('port', 'string'), F_('path', 'string'), F_('interval', 'duration'), F_('scheme', 'string')])])],
    'prometheusrules.monitoring.coreos.com':[F_('spec', 'object', '', true, [F_('groups', '[]object', 'Rule groups', true, [F_('name', 'string', '', true), F_('interval', 'duration'), F_('rules', '[]object', '', false, [F_('alert', 'string'), F_('expr', 'string', 'PromQL expression', true), F_('for', 'duration'), F_('labels', 'map[string]string')])])])]
  };
  function schemaHtml(list, depth){
    return '<ul class="schema">' + list.map(n => {
      const head = '<span class="fname">' + esc(n.name) + '</span><span class="ftype">' + esc(n.type) + '</span>' + (n.req ? '<span class="freq">required</span>' : '');
      const desc = n.desc ? '<span class="fdesc">' + esc(n.desc) + '</span>' : '';
      if (n.kids) return '<li><details' + (depth < 1 ? ' open' : '') + '><summary>' + head + '</summary>' + desc + schemaHtml(n.kids, depth + 1) + '</details></li>';
      return '<li><div class="leaf">' + head + '</div>' + (n.desc ? '<span class="fdesc" style="margin-left:18px">' + esc(n.desc) + '</span>' : '') + '</li>';
    }).join('') + '</ul>';
  }
  function crdOverview(f){
    const r = f.r, k = CRD_KIND[r.name];
    return '<dl class="kv-list">' + kv('Kind', '<span class="mono">' + esc(crdKind(r.name)) + '</span>') + kv('Group', '<span class="mono">' + esc(r.group) + '</span>') + kv('Plural', '<span class="mono">' + esc(r.name.split('.')[0]) + '</span>') +
      kv('Scope', esc(r.scope)) + kv('Versions', '<span class="chip-list"><span class="label-chip">' + esc(r.versions) + '</span></span>') +
      kv('Instances', k ? '<button type="button" class="link-btn" data-drill="' + esc(r.name) + '">' + cnt(r.instances) + ' resources →</button>' : '0') + kv('Established', pill('True', 'success')) + kv('Created', fmtAge(r.age) + ' ago') + '</dl>' +
      '<div class="inspector-eyebrow">Labels</div>' + chipsOf(labelsFor('crds', r));
  }
  function helmPanel(f){
    const items = helmResources(f.r);
    if (!items.length) return '<p class="muted-note">No resources are recorded for this release.</p>';
    let ok = 0, warn = 0, bad = 0;
    items.forEach(x => { const st = statusOf(x.kind, x.r), t = st ? st[1] : 'success'; if (t === 'danger') bad++; else if (t === 'warn') warn++; else ok++; });
    const seg = (n, col) => n ? '<span style="width:' + n / items.length * 100 + '%;background:' + col + '"></span>' : '';
    const by = {}, order = [];
    items.forEach(x => { if (!by[x.kind]) { by[x.kind] = []; order.push(x.kind); } by[x.kind].push(x); });
    return '<div class="stackbar" role="img" aria-label="Release health">' + seg(ok, 'var(--success)') + seg(warn, 'var(--warn)') + seg(bad, 'var(--danger)') + '</div>' +
      '<div class="seg-legend"><span>' + items.length + ' resources in ' + order.length + ' kinds</span><span><i style="background:var(--success)"></i>healthy <b>' + ok + '</b></span>' + (warn ? '<span><i style="background:var(--warn)"></i>progressing <b>' + warn + '</b></span>' : '') + (bad ? '<span><i style="background:var(--danger)"></i>failing <b>' + bad + '</b></span>' : '') + '</div>' +
      order.map(k => '<div class="inspector-eyebrow">' + esc(KINDS[k].label) + ' <span class="muted-note">' + by[k].length + '</span></div><ul class="inst-list">' + by[k].map(x => { const st = statusOf(k, x.r); return '<li><button type="button" class="inst-row" data-goto="' + esc(x.r.id) + '">' + ICON(KINDS[k].icon) + '<span class="mono">' + esc(x.r.name) + '</span>' + (st ? pill(st[0], st[1]) : '') + '</button></li>'; }).join('') + '</ul>').join('');
  }
  function resourcesPanel(f){
    if (f.kind === 'helm') return helmPanel(f);
    const r = f.r, k = CRD_KIND[r.name], list = k ? DATA[k] : [];
    if (!list.length) return '<p class="muted-note">No instances of this resource exist yet.</p>';
    return '<ul class="inst-list">' + list.map(x => { const st = statusOf(k, x); return '<li><button type="button" class="inst-row" data-goto="' + esc(x.id) + '">' + ICON(KINDS[k].icon) + '<span class="mono">' + esc((x.ns ? x.ns + '/' : '') + x.name) + '</span>' + (st ? pill(st[0], st[1]) : '') + '</button></li>'; }).join('') + '</ul>' +
      '<button type="button" class="btn btn-secondary" data-drill="' + esc(r.name) + '" style="min-height:44px">Open full list ' + ICON('arrow') + '</button>';
  }

  /* ---- cluster overview */
  const POD_TONES = {Running:'var(--success)', Completed:'var(--accent)', Pending:'var(--warn)', ContainerCreating:'var(--warn)', CrashLoopBackOff:'var(--danger)'};
  function resolveObj(s){
    const m = s.match(/^(pod|deployment|statefulset|daemonset|job)\/(.+)$/);
    if (!m) return null;
    const k = {pod:'pods', deployment:'deployments', statefulset:'statefulsets', daemonset:'daemonsets', job:'jobs'}[m[1]];
    const hit = DATA[k].find(x => x.name === m[2]);
    return hit ? hit.id : null;
  }
  /* ------------------------------------------- clusters, account, settings */
  const lsGet = (k, d) => { try { const v = localStorage.getItem(k); return v == null ? d : v; } catch (err) { return d; } };
  const lsSet = (k, v) => { try { localStorage.setItem(k, v); } catch (err) { /* private mode */ } };
  const CLUSTERS = [
    {id:'prod-us-east-1', ver:'v1.29.4', region:'us-east-1', src:'~/.kube/config', down:false},
    {id:'staging-eu-west-1', ver:'v1.29.2', region:'eu-west-1', src:'~/.kube/config', down:false},
    {id:'prod-ap-south-1', ver:'v1.28.9', region:'ap-south-1', src:'~/.kube/config', down:true},
    {id:'dev-kind', ver:'v1.30.0', region:'local', src:'~/work/clusters/', down:false},
    {id:'minikube', ver:'v1.30.0', region:'local', src:'~/work/clusters/', down:false}
  ];
  const KUBE_PATHS = ['~/.kube/config', '~/work/clusters/'];
  S.cluster = CLUSTERS.some(c => c.id === lsGet('openkite-cluster', '')) ? lsGet('openkite-cluster', '') : 'prod-us-east-1';
  const clusterBtn = $('#clusterBtn'), clusterMenu = $('#clusterMenu'), userMenu = $('#userMenu');
  const curCluster = () => CLUSTERS.find(c => c.id === S.cluster) || CLUSTERS[0];

  function applyCluster(){
    const c = curCluster();
    $('#clusterName').textContent = c.id;
    $('#clusterMeta').textContent = c.ver + ' · ' + c.region;
    $('#crumbClusterName').textContent = c.id;
    $('#crumbCluster').setAttribute('aria-label', 'Open cluster overview for ' + c.id);
    $('#ovSub').textContent = c.id + ' · Kubernetes ' + c.ver + ' · 8 nodes across 3 availability zones.';
    $('#connLabel').textContent = c.down ? 'Unreachable' : 'Connected';
    $$('.cluster-btn .dot, .crumb-cluster .dot, .foot-meta .dot').forEach(d => d.classList.toggle('down', c.down));
  }
  function placePop(el, anchor, right){
    el.hidden = false;
    const r = anchor.getBoundingClientRect(), w = el.offsetWidth, h = el.offsetHeight;
    el.style.left = Math.max(8, Math.min(window.innerWidth - w - 8, right ? r.right - w : r.left)) + 'px';
    el.style.top = (r.bottom + h + 8 > window.innerHeight ? Math.max(8, r.top - h - 4) : r.bottom + 4) + 'px';
  }
  function closePops(){
    const was = !clusterMenu.hidden || !userMenu.hidden || !navFilterMenu.hidden || !crumbMenu.hidden || !nsMenu.hidden || !colMenu.hidden || !refMenu.hidden || !inspMenu.hidden;
    inspMenu.hidden = true; $('#inspMore').setAttribute('aria-expanded', 'false');
    refMenu.hidden = true; $$('[data-argo-rmenu]').forEach(b => b.setAttribute('aria-expanded', 'false'));
    colMenu.hidden = true; $('#colBtn').setAttribute('aria-expanded', 'false');
    clusterMenu.hidden = true; userMenu.hidden = true; navFilterMenu.hidden = true; crumbMenu.hidden = true; nsMenu.hidden = true;
    ['navFilterBtn', 'crumbSectionBtn', 'crumbCurrentBtn', 'nsPillBtn'].forEach(id => $('#' + id).setAttribute('aria-expanded', 'false'));
    $$('.ns-trigger').forEach(b => b.setAttribute('aria-expanded', 'false'));
    clusterBtn.setAttribute('aria-expanded', 'false'); $('#profileBtn').setAttribute('aria-expanded', 'false');
    return was;
  }
  function clusterListHtml(q){
    q = (q || '').trim().toLowerCase();
    const list = CLUSTERS.filter(c => !q || (c.id + ' ' + c.region + ' ' + c.src).toLowerCase().includes(q));
    if (!list.length) return '<p class="muted-note cm-empty">No clusters match “' + esc(q) + '”.</p>';
    const groups = Array.from(new Set(list.map(c => c.src)));
    return groups.map(src => '<div class="menu-group">' + esc(src) + '</div>' + list.filter(c => c.src === src).map(c =>
      '<button type="button" class="cm-item" role="menuitemradio" aria-checked="' + (c.id === S.cluster) + '" data-cluster="' + esc(c.id) + '"><span class="dot' + (c.down ? ' down' : '') + '"></span><span class="cm-text"><b>' + esc(c.id) + '</b><small>' + esc(c.ver + ' · ' + c.region) + (c.down ? ' · unreachable' : '') + '</small></span>' + (c.id === S.cluster ? ICON('check') : '') + '</button>').join('')).join('');
  }
  function openClusterMenu(anchor){
    closePops();
    clusterMenu.innerHTML = '<div class="cm-search"><label class="search-field">' + ICON('search') + '<input type="search" id="clusterSearch" placeholder="Search clusters…" aria-label="Search clusters" autocomplete="off"></label></div><div class="cm-list" id="clusterList">' + clusterListHtml('') + '</div>' +
      '<div class="cm-foot"><button type="button" class="cm-act" data-cm-overview>' + ICON('cluster') + 'Open cluster overview</button><button type="button" class="cm-act" data-kube-dialog>' + ICON('plus') + 'Add kubeconfig directory…</button><button type="button" class="cm-act" data-toast="Paste a kubeconfig or connect via cloud provider in the full product">' + ICON('cluster') + 'Add cluster…</button></div>';
    placePop(clusterMenu, anchor, false);
    anchor.setAttribute('aria-expanded', 'true');
    const s = $('#clusterSearch'); if (s && window.matchMedia && window.matchMedia('(hover:hover)').matches) s.focus();
  }
  clusterMenu.addEventListener('input', e => { if (e.target.id === 'clusterSearch') $('#clusterList').innerHTML = clusterListHtml(e.target.value); });
  clusterMenu.addEventListener('click', e => {
    const it = e.target.closest('[data-cluster]');
    if (it) {
      S.cluster = it.dataset.cluster; lsSet('openkite-cluster', S.cluster);
      closePops(); applyCluster();
      showToast(curCluster().down ? curCluster().id + ' is unreachable — showing last known state' : 'Switched to ' + curCluster().id);
      return;
    }
    if (e.target.closest('[data-cm-overview]')) { closePops(); setView('overview'); return; }
    if (e.target.closest('[data-kube-dialog]')) { closePops(); openKubeDialog(); }
    const tt = e.target.closest('[data-toast]'); if (tt) { closePops(); showToast(tt.dataset.toast); }
  });
  clusterBtn.addEventListener('click', () => { if (clusterMenu.hidden) openClusterMenu(clusterBtn); else closePops(); });
  $('#profileBtn').addEventListener('click', () => {
    if (!userMenu.hidden) return void closePops();
    closePops();
    userMenu.innerHTML = '<div class="um-head"><span class="avatar sm" aria-hidden="true">EK</span><span class="profile-text"><b>Emma Kline</b><small>emma@acme.io</small></span></div>' +
      '<button type="button" class="rm-item" role="menuitem" data-toast="Profile opens here">' + ICON('user') + '<span class="rm-text">Profile</span></button>' +
      '<button type="button" class="rm-item" role="menuitem" data-open-settings>' + ICON('settings') + '<span class="rm-text">Settings</span></button>' +
      '<button type="button" class="rm-item" role="menuitem" data-shortcuts>' + ICON('terminal') + '<span class="rm-text">Keyboard shortcuts</span></button>' +
      '<button type="button" class="rm-item" role="menuitem" data-theme-toggle aria-pressed="' + (document.documentElement.dataset.theme === 'dark') + '">' + ICON('moon') + '<span class="rm-text">Dark mode</span><span class="switch" aria-hidden="true"></span></button>' +
      '<button type="button" class="rm-item" role="menuitem" data-toast="Signed out (demo)">' + ICON('arrow') + '<span class="rm-text">Sign out</span></button>';
    placePop(userMenu, $('#profileBtn'), false);
    $('#profileBtn').setAttribute('aria-expanded', 'true');
  });
  userMenu.addEventListener('click', e => {
    if (e.target.closest('[data-open-settings]')) { closePops(); openSettings(); }
    else if (e.target.closest('[data-shortcuts]')) { closePops(); openShortcuts(); }
    else { const t = e.target.closest('[data-toast]'); if (t) showToast(t.dataset.toast); if (t || e.target.closest('[data-theme-toggle]')) closePops(); }
  });
  document.addEventListener('click', e => {
    // a click on an item that the menu has just re-rendered is detached by now; that is not an outside click
    if (e.target.isConnected && (!clusterMenu.hidden || !userMenu.hidden || !navFilterMenu.hidden || !crumbMenu.hidden || !nsMenu.hidden || !colMenu.hidden || !refMenu.hidden || !inspMenu.hidden) && !e.target.closest('#clusterMenu,#userMenu,#clusterBtn,#crumbCluster,#profileBtn,[data-cluster-open],#navFilterMenu,#navFilterBtn,#crumbMenu,#crumbSectionBtn,#crumbCurrentBtn,#nsMenu,#nsPillBtn,.ns-trigger,[data-nsopen],#colMenu,#colBtn,#refMenu,[data-argo-rmenu],#inspMenu,#inspMore')) closePops();
    if (e.target.closest('#settingsBtn,[data-open-settings]') && !e.target.closest('#userMenu')) { closeMenu(); openSettings(); }
    const co = e.target.closest('[data-cluster-open]'); if (co) { closeMenu(); openClusterMenu($('#crumbCluster')); }
  });

  /* kubeconfig search paths */
  const ctxCount = p => CLUSTERS.filter(c => c.src === p).length;
  function kubeInner(){
    return '<ul class="kube-list">' + KUBE_PATHS.map(p => '<li><span class="mono">' + esc(p) + '</span><span class="kube-count">' + ctxCount(p) + ' context' + (ctxCount(p) === 1 ? '' : 's') + '</span><button type="button" class="kube-rm" data-kube-rm="' + esc(p) + '" aria-label="Remove ' + esc(p) + '">' + ICON('close') + '</button></li>').join('') + '</ul>' +
      '<div class="kube-add"><label class="field"><span>Add directory or file</span><input type="text" id="kubeInput" placeholder="~/path/to/kubeconfigs" autocomplete="off" spellcheck="false"></label><button type="button" class="btn btn-primary" data-kube-add>Add</button></div>' +
      '<p class="muted-note">Every kubeconfig file in a directory is scanned for contexts.</p>';
  }
  function addKubePath(){
    const inp = $('#kubeInput'), p = (inp.value || '').trim();
    if (!p) return inp.focus();
    if (KUBE_PATHS.indexOf(p) >= 0) return showToast('Already scanning ' + p);
    KUBE_PATHS.push(p);
    const base = p.replace(/\/+$/, '').split('/').pop().replace(/^\./, '') || 'custom';
    ['prod', 'staging'].forEach(env => CLUSTERS.push({id:base + '-' + env, ver:'v1.29.' + (env === 'prod' ? 4 : 1), region:'custom', src:p, down:false}));
    $('#kubeBlock').innerHTML = kubeInner();
    showToast('Found 2 contexts in ' + p);
  }
  function openKubeDialog(){
    confirmDlg({title:'Kubeconfig search paths', html:'<div id="kubeBlock">' + kubeInner() + '</div>', ok:'Done', wide:true, onOk:() => {}});
  }
  function shortcutsHtml(){
    const row = (k, d) => '<li><kbd>' + k + '</kbd><span>' + d + '</span></li>';
    return '<ul class="kbd-list">' + row('Ctrl + `', 'Show or hide logs &amp; terminal') + row('Esc', 'Close menus, dialogs and the details panel') + row('↑ ↓', 'Browse terminal history') + row('Enter', 'Open the focused row') + '</ul>';
  }
  function openShortcuts(){ confirmDlg({title:'Keyboard shortcuts', html:shortcutsHtml(), ok:'Close', onOk:() => {}}); }
  function openSettings(){
    const dark = document.documentElement.dataset.theme === 'dark';
    confirmDlg({title:'Settings', wide:true, ok:'Done', onOk:() => {}, html:
      '<div class="set-sec"><h4>Appearance</h4><div class="seg" role="group" aria-label="Theme"><button type="button" data-set-theme="light" aria-pressed="' + !dark + '">Light</button><button type="button" data-set-theme="dark" aria-pressed="' + dark + '">Dark</button></div></div>' +
      '<div class="set-sec"><h4>Menu layout</h4><div class="seg" role="group" aria-label="Menu layout"><button type="button" data-set-layout="tree" aria-pressed="' + (S.navLayout === 'tree') + '">Tree</button><button type="button" data-set-layout="grid" aria-pressed="' + (S.navLayout === 'grid') + '">Tree-grid</button></div></div>' +
      '<div class="set-sec"><h4>Default time range</h4><div id="setRange">' + OK.rangeTabs(S.range) + '</div></div>' +
      '<div class="set-sec"><h4>Overview</h4><button type="button" class="btn btn-secondary" data-ov-customize>Customize overview…</button></div>' +
      '<div class="set-sec"><h4>Kubeconfig search paths</h4><div id="kubeBlock">' + kubeInner() + '</div></div>'});
  }
  $('#dlgBody').addEventListener('click', e => {
    if (e.target.closest('[data-kube-add]')) return addKubePath();
    const rm = e.target.closest('[data-kube-rm]');
    if (rm) {
      const p = rm.dataset.kubeRm;
      KUBE_PATHS.splice(KUBE_PATHS.indexOf(p), 1);
      for (let i = CLUSTERS.length - 1; i >= 0; i--) if (CLUSTERS[i].src === p) CLUSTERS.splice(i, 1);
      if (!CLUSTERS.some(c => c.id === S.cluster)) { S.cluster = CLUSTERS[0] ? CLUSTERS[0].id : ''; applyCluster(); }
      $('#kubeBlock').innerHTML = kubeInner();
      return showToast('Stopped scanning ' + p);
    }
    const sl = e.target.closest('[data-set-layout]');
    if (sl) { setNavLayout(sl.dataset.setLayout); $$('[data-set-layout]').forEach(b => b.setAttribute('aria-pressed', String(b === sl))); return; }
    const th = e.target.closest('[data-set-theme]');
    if (th) { applyTheme(th.dataset.setTheme, true); $$('[data-set-theme]').forEach(b => b.setAttribute('aria-pressed', String(b === th))); return; }
    if (e.target.closest('[data-range]')) { const sr = $('#setRange'); if (sr) setTimeout(() => { sr.innerHTML = OK.rangeTabs(S.range); }, 0); return; }
    if (e.target.closest('[data-ov-customize]')) { closeDlg(); S.ovEdit = true; setView('overview'); }
  });
  $('#dlgBody').addEventListener('keydown', e => { if (e.key === 'Enter' && e.target.id === 'kubeInput') { e.preventDefault(); e.stopPropagation(); addKubePath(); } });

  /* ------------------------------------------------ customizable overview */
  const OV_DEFS = {
    attention:{title:'Needs attention', desc:'Failing and pending pods, high restarts, degraded workloads, unhealthy nodes and out-of-sync apps.', size:'full'},
    stats:{title:'Key numbers', desc:'Node, pod, deployment and Argo CD totals with trend lines.', size:'full'},
    usage:{title:'Cluster usage', desc:'CPU, memory, network and filesystem over time.', size:'full'},
    problems:{title:'Pending & failing pods', desc:'Pods that are not running, with restart counts.', size:'half'},
    restarts:{title:'High pod restarts', desc:'Pods that restart most often. Set your own threshold.', size:'half'},
    capacity:{title:'Capacity', desc:'Requests against allocatable CPU, memory, pods and storage.', size:'half'},
    nodes:{title:'Nodes', desc:'Per-node CPU and memory utilisation.', size:'half'},
    health:{title:'Workload health', desc:'Pods by status and the busiest namespaces.', size:'half'},
    argo:{title:'Argo CD sync', desc:'Applications that are out of sync or unhealthy.', size:'half'},
    events:{title:'Recent events', desc:'Latest cluster events with a warnings filter.', size:'full'}
  };
  const OV_ORDER = ['attention', 'usage', 'problems', 'restarts', 'capacity', 'nodes', 'health', 'argo', 'events', 'stats'];
  function loadOv(){
    let saved = {};
    try { saved = JSON.parse(lsGet('openkite-overview', '{}')) || {}; } catch (err) { saved = {}; }
    const order = (saved.order || []).filter(id => OV_DEFS[id]);
    OV_ORDER.forEach(id => { if (order.indexOf(id) < 0) order.push(id); });
    return {order, hidden:saved.hidden || ['stats'], size:saved.size || {}, lane:saved.lane || {}, thr:saved.thr || 3};
  }
  S.ov = loadOv(); S.ovEdit = false;
  const saveOv = () => lsSet('openkite-overview', JSON.stringify(S.ov));
  const ovSize = id => S.ov.size[id] || OV_DEFS[id].size;
  /* Layout model: full-width cards, plus "bands" made of two independent lanes (left / right) of half-width cards.
     Each lane stacks on its own, so a tall half card can sit beside several short ones. */
  const OV_WEIGHT = {usage:3, health:2, nodes:2, events:2};   // rough heights, used to balance the two lanes by default
  function ovGroups(){
    const vis = S.ov.order.filter(id => S.ov.hidden.indexOf(id) < 0), groups = [];
    let band = null, wl = 0, wr = 0;
    vis.forEach(id => {
      if (ovSize(id) === 'full') { groups.push({type:'full', id}); band = null; return; }
      if (!band) { band = {type:'band', L:[], R:[]}; groups.push(band); wl = wr = 0; }
      let lane = S.ov.lane[id];
      if (lane !== 'L' && lane !== 'R') lane = wl <= wr ? 'L' : 'R';
      band[lane].push(id);
      if (lane === 'L') wl += OV_WEIGHT[id] || 1; else wr += OV_WEIGHT[id] || 1;
    });
    return groups;
  }
  function ovCommit(groups){
    const flat = [];
    groups.forEach(g => {
      if (g.type === 'full') { flat.push(g.id); return; }
      g.L.forEach(id => { flat.push(id); S.ov.lane[id] = 'L'; S.ov.size[id] = 'half'; });
      g.R.forEach(id => { flat.push(id); S.ov.lane[id] = 'R'; S.ov.size[id] = 'half'; });
    });
    S.ov.order = flat.concat(S.ov.order.filter(x => flat.indexOf(x) < 0));
    saveOv();
  }
  function ovMove(id, dir){
    const groups = ovGroups(), gi = groups.findIndex(g => g.type === 'full' ? g.id === id : g.L.indexOf(id) >= 0 || g.R.indexOf(id) >= 0);
    if (gi < 0) return;
    const g = groups[gi];
    if (g.type === 'full') { const j = gi + dir; if (j < 0 || j >= groups.length) return; groups[gi] = groups[j]; groups[j] = g; }
    else { const lane = g.L.indexOf(id) >= 0 ? g.L : g.R, i = lane.indexOf(id), j = i + dir; if (j < 0 || j >= lane.length) return; lane[i] = lane[j]; lane[j] = id; }
    ovCommit(groups);
  }
  function ovLane(id){
    const groups = ovGroups(), g = groups.find(x => x.type === 'band' && (x.L.indexOf(id) >= 0 || x.R.indexOf(id) >= 0));
    if (!g) return;
    const from = g.L.indexOf(id) >= 0 ? g.L : g.R, to = from === g.L ? g.R : g.L;
    from.splice(from.indexOf(id), 1); to.push(id);
    ovCommit(groups);
  }
  const PROBLEM = new Set(['Pending', 'ContainerCreating', 'CrashLoopBackOff', 'Failed']);
  const FAILING = new Set(['CrashLoopBackOff', 'Failed']);

  function openKindFiltered(kind, label, fn){
    setView('workloads', kind);
    S.flt = {label, fn};
    renderChips(); renderRows();
  }
  const panelHtml = (id, title, body, extraHead) => '<div class="panel" data-od-id="overview-' + id + '"><div class="panel-head"><h3>' + esc(title) + '</h3>' + (extraHead || '') + '</div>' + body + '</div>';
  const emptyOk = msg => '<div class="all-clear">' + ICON('check') + '<span>' + esc(msg) + '</span></div>';

  const OV_RENDER = {
    attention(){
      const thr = S.ov.thr, failing = NSD('pods').filter(p => FAILING.has(p.status)), pending = NSD('pods').filter(p => p.status === 'Pending' || p.status === 'ContainerCreating'),
        hi = NSD('pods').filter(p => p.restarts >= thr), degraded = NSD('deployments').filter(d => d.have < d.want), badNodes = DATA.nodes.filter(n => n.status !== 'Ready'),
        oos = NSD('argoapps').filter(a => a.sync !== 'Synced' || a.health !== 'Healthy'), warns = NSD('events').filter(e => e.type === 'Warning');
      const tile = (n, label, tone, act) => '<button type="button" class="att-tile ' + (n ? tone : 'ok') + '" data-act="' + act + '"><span class="att-n">' + n + '</span><span class="att-l">' + esc(label) + '</span></button>';
      return panelHtml('attention', 'Needs attention', '<div class="att-grid">' + tile(failing.length, 'Failing pods', 'danger', 'failing') + tile(pending.length, 'Pending pods', 'warn', 'pending') + tile(hi.length, 'Restarts ≥ ' + thr, 'warn', 'restarts') +
        tile(degraded.length, 'Degraded deployments', 'danger', 'degraded') + tile(badNodes.length, 'Nodes not ready', 'warn', 'nodes') + tile(oos.length, 'Apps out of sync', 'warn', 'argo') + tile(warns.length, 'Warning events', 'warn', 'warnings') + '</div>');
    },
    stats(){
      const range = S.range, spk = (seed, base, amp, color) => OK.spark(OK.series(seed + range, 48, base, amp, base * 0.4), color);
      const stat = (label, val, sub, sp) => '<div class="stat-card"><span class="stat-label">' + label + '</span><span class="stat-value">' + val + '<small>' + sub + '</small></span>' + sp + '</div>';
      return '<div class="ov-stats" data-od-id="overview-stats">' + stat('Nodes', DATA.nodes.length, DATA.nodes.filter(n => n.status === 'Ready').length + ' ready', spk('n', 6, 1.5, C.cpu)) + stat('Pods', NSD('pods').length, NSD('pods').filter(p => p.status === 'Running').length + ' running', spk('p', 20, 6, C.mem)) +
        stat('Deployments', NSD('deployments').length, NSD('deployments').filter(d => d.have < d.want).length + ' degraded', spk('d', 10, 3, C.rx)) + stat('Argo CD apps', NSD('argoapps').length, NSD('argoapps').filter(a => a.sync !== 'Synced').length + ' out of sync', spk('a', 12, 4, 'var(--argo)')) + '</div>';
    },
    usage(){
      if (!NSD('pods').length) return panelHtml('usage', 'Cluster usage', emptyOk('No pods in the selected namespaces'));
      const agg = aggMetrics(NSD('pods').map(p => metricsOf('pods', p, S.range)), S.range);
      return panelHtml('usage', 'Cluster usage', '<div class="panel-body"><div class="chart-grid">' + chartSet(agg, 'cluster') + '</div></div>', OK.rangeTabs(S.range));
    },
    problems(){
      const list = NSD('pods').filter(p => PROBLEM.has(p.status)).sort((a, b) => b.restarts - a.restarts);
      const body = list.length ? '<div class="prob-list">' + list.map(p => '<button type="button" class="prob-row" data-goto="' + esc(p.id) + '">' + pill(p.status, podTone(p.status)) + '<span class="mono">' + esc(p.name) + '</span><span class="prob-ns">' + esc(p.ns) + '</span><span class="prob-r" title="Restarts">↻ ' + p.restarts + '</span></button>').join('') + '</div>' : emptyOk('No pending or failing pods');
      return panelHtml('problems', 'Pending & failing pods', body, list.length ? '<button type="button" class="link-back" data-act="problems-all">View all ' + ICON('arrow') + '</button>' : '');
    },
    restarts(){
      const thr = S.ov.thr, list = NSD('pods').filter(p => p.restarts >= thr).sort((a, b) => b.restarts - a.restarts), mx = list[0] ? list[0].restarts : 1;
      const sel = '<label class="thr"><span>Restarts ≥</span><select data-ov-thr aria-label="Restart threshold">' + [1, 3, 5, 10].map(n => '<option' + (n === thr ? ' selected' : '') + '>' + n + '</option>').join('') + '</select></label>';
      const body = list.length ? '<div class="rank-list" style="margin:0;padding:6px 8px">' + list.map(p => '<button type="button" class="rank-row" data-goto="' + esc(p.id) + '"><span class="mono">' + esc(p.name) + '</span><span class="hbar"><span class="track"><span class="fill danger" style="width:' + Math.max(6, p.restarts / mx * 100) + '%"></span></span><span class="num">' + p.restarts + '×</span></span></button>').join('') + '</div>' : emptyOk('No pod has restarted ' + thr + ' times or more');
      return panelHtml('restarts', 'High pod restarts', body, sel);
    },
    capacity(){
      return panelHtml('capacity', 'Capacity', '<div class="panel-body"><div class="gauge-row">' + OK.gauge(60, 'CPU requests', '38.4 / 64 cores', C.cpu) + OK.gauge(46, 'Memory requests', '118 / 256 GiB', C.mem) + OK.gauge(14, 'Pod slots', '124 / 880', C.rx) + OK.gauge(64, 'Storage', '640 / 1000 GiB', C.fr) + '</div></div>');
    },
    nodes(){
      return panelHtml('nodes', 'Nodes', '<div class="node-list">' + DATA.nodes.map(n => {
        const cordoned = /Disabled/.test(n.status);
        return '<button type="button" class="node-row" data-goto="' + esc(n.id) + '"><span class="mono">' + esc(n.name) + '</span>' + hbar(parseInt(n.cpu, 10) || 0, '') + hbar(parseInt(n.mem, 10) || 0, 'violet') + pill(cordoned ? 'Cordoned' : n.roles, cordoned ? 'warn' : 'success') + '</button>';
      }).join('') + '</div>', '<span class="muted-note">CPU · Memory</span>');
    },
    health(){
      const counts = {};
      NSD('pods').forEach(p => { counts[p.status] = (counts[p.status] || 0) + 1; });
      const ms = NSD('pods').map(p => metricsOf('pods', p)), rank = DATA.namespaces.map(n => ({n:n.name, c:NSD('pods').reduce((a, p, i) => a + (p.ns === n.name ? lastOf(ms[i].cpu.data) : 0), 0)})).filter(x => x.c > 0).sort((a, b) => b.c - a.c), mx = rank[0] ? rank[0].c : 1;
      return panelHtml('health', 'Workload health', '<div class="panel-body"><div class="stackbar" role="img" aria-label="Pods by status">' + Object.keys(counts).map(s => '<span style="width:' + counts[s] / NSD('pods').length * 100 + '%;background:' + (POD_TONES[s] || 'var(--muted)') + '"></span>').join('') + '</div><div class="seg-legend">' +
        Object.keys(counts).map(s => '<span><i style="background:' + (POD_TONES[s] || 'var(--muted)') + '"></i>' + esc(s) + ' <b>' + counts[s] + '</b></span>').join('') + '</div><div class="inspector-eyebrow" style="margin-top:18px">Top namespaces by CPU</div><div class="rank-list" style="margin-top:0">' +
        rank.map(x => '<button type="button" class="rank-row" data-ns-open="' + esc(x.n) + '"><span class="mono">' + esc(x.n) + '</span>' + hbar(x.c / mx * 100, '') + '</button>').join('') + '</div></div>');
    },
    argo(){
      const bad = NSD('argoapps').filter(a => a.sync !== 'Synced' || a.health !== 'Healthy');
      const body = bad.length ? '<div class="prob-list">' + bad.map(a => '<button type="button" class="prob-row" data-goto="' + esc(a.id) + '">' + pill(a.sync, tone(a.sync)) + '<span class="mono">' + esc(a.name) + '</span>' + pill(a.health, tone(a.health)) + '</button>').join('') + '</div>' : emptyOk('All applications are synced and healthy');
      return panelHtml('argo', 'Argo CD sync', body, '<span class="muted-note">' + (NSD('argoapps').length - bad.length) + ' of ' + NSD('argoapps').length + ' healthy</span>');
    },
    events(){
      const evs = NSD('events').slice().sort((a, b) => a.age - b.age).filter(e => S.evFilter === 'all' || e.type === 'Warning');
      return panelHtml('events', 'Recent events', '<table class="ev-table"><tbody>' + evs.map(e => { const id = resolveObj(e.name); return '<tr' + (id ? ' class="go" tabindex="0" data-goto="' + esc(id) + '"' : '') + '><td style="width:1%">' + pill(e.type, e.type === 'Warning' ? 'warn' : 'success') + '</td><td><b>' + esc(e.reason) + '</b></td><td class="col-obj obj">' + esc(e.name) + '</td><td class="col-msg msg">' + esc(e.message) + '</td><td class="age">' + fmtAge(e.age) + '</td></tr>'; }).join('') + '</tbody></table>',
        '<div class="chip-row"><button type="button" class="chip' + (S.evFilter === 'all' ? ' active' : '') + '" data-evf="all">All</button><button type="button" class="chip' + (S.evFilter === 'Warning' ? ' active' : '') + '" data-evf="Warning">Warnings</button><button type="button" class="link-back" data-kindgo="events">View all ' + ICON('arrow') + '</button></div>');
    }
  };
  function renderOverview(){
    const el = $('#ovBody');
    if (!el) return;
    const edit = S.ovEdit, vis = S.ov.order.filter(id => S.ov.hidden.indexOf(id) < 0), hidden = S.ov.order.filter(id => S.ov.hidden.indexOf(id) >= 0);
    $('#ovCustomize').setAttribute('aria-pressed', String(edit));
    $('#ovCustomize').lastElementChild.textContent = edit ? 'Done' : 'Customize';
    let h = '';
    if (S.ns.size) h += '<div class="ov-ns-note" role="status"><span>Showing namespaces: <b>' + Array.from(S.ns).map(esc).join(', ') + '</b></span><button type="button" class="icon-btn x-clear" data-act="clear-ns" aria-label="Clear namespace filter" title="Clear filter">' + ICON('close') + '</button></div>';
    if (edit) h += '<div class="ov-edit-bar" role="status"><span><b>Customizing overview.</b> Reorder, resize or hide widgets. Changes are saved on this device.</span><button type="button" class="btn btn-secondary" data-act="reset">Reset to default</button><button type="button" class="btn btn-primary" data-act="done">Done</button></div>';
    const groups = ovGroups();
    const barFor = id => !edit ? '' : '<div class="ov-w-bar"><span class="ov-w-title">' + esc(OV_DEFS[id].title) + '</span>' +
      '<button type="button" class="ovb" data-ovw="up" data-id="' + id + '" aria-label="Move ' + esc(OV_DEFS[id].title) + ' up"><svg class="icon" style="transform:rotate(-90deg)"><use href="#i-chevron"/></svg></button>' +
      '<button type="button" class="ovb" data-ovw="down" data-id="' + id + '" aria-label="Move ' + esc(OV_DEFS[id].title) + ' down"><svg class="icon" style="transform:rotate(90deg)"><use href="#i-chevron"/></svg></button>' +
      (ovSize(id) === 'half' ? '<button type="button" class="ovb" data-ovw="lane" data-id="' + id + '" aria-label="Move ' + esc(OV_DEFS[id].title) + ' to the other column" title="Other column">' + ICON('swap') + '</button>' : '') +
      '<button type="button" class="ovb txt" data-ovw="size" data-id="' + id + '" aria-label="Toggle width of ' + esc(OV_DEFS[id].title) + '">' + (ovSize(id) === 'full' ? 'Full' : 'Half') + '</button>' +
      '<button type="button" class="ovb" data-ovw="hide" data-id="' + id + '" aria-label="Hide ' + esc(OV_DEFS[id].title) + '">' + ICON('close') + '</button></div>';
    const card = id => '<section class="ov-w ' + (ovSize(id) === 'full' ? 'full' : 'half') + '" data-w="' + id + '" data-pref="' + ovSize(id) + '"><button type="button" class="ov-grip" data-grip="' + id + '" aria-label="Drag to reorder ' + esc(OV_DEFS[id].title) + '. Arrow keys also move it." title="Drag to reorder"><svg viewBox="0 0 10 18" aria-hidden="true"><g fill="currentColor"><circle cx="2.5" cy="3" r="1.3"/><circle cx="7.5" cy="3" r="1.3"/><circle cx="2.5" cy="9" r="1.3"/><circle cx="7.5" cy="9" r="1.3"/><circle cx="2.5" cy="15" r="1.3"/><circle cx="7.5" cy="15" r="1.3"/></g></svg></button><div class="ov-body">' + barFor(id) + OV_RENDER[id]() + '</div></section>';
    h += '<div class="ov-widgets' + (edit ? ' editing' : '') + '">' + groups.map(g => g.type === 'full' ? card(g.id) :
      '<div class="ov-band" data-band>' + ['L', 'R'].map(k => '<div class="ov-lane" data-lane="' + k + '">' + g[k].map(card).join('') + '<div class="ov-lane-end" aria-hidden="true"></div></div>').join('') + '</div>').join('') + '</div>';
    if (edit) h += '<div class="ov-tray"><h3>Available widgets</h3>' + (hidden.length ? hidden.map(id => '<div class="ov-tray-item"><div><b>' + esc(OV_DEFS[id].title) + '</b><small>' + esc(OV_DEFS[id].desc) + '</small></div><button type="button" class="btn btn-secondary" data-ovw="add" data-id="' + id + '">' + ICON('plus') + 'Add</button></div>').join('') : '<p class="muted-note">Every widget is on your overview.</p>') + '</div>';
    el.innerHTML = h;
  }
  $('#ovBody').addEventListener('click', e => {
    const w = e.target.closest('[data-ovw]');
    if (w) {
      const id = w.dataset.id, op = w.dataset.ovw;
      if (op === 'up') ovMove(id, -1); else if (op === 'down') ovMove(id, 1); else if (op === 'lane') ovLane(id);
      else if (op === 'size') S.ov.size[id] = ovSize(id) === 'full' ? 'half' : 'full';
      else if (op === 'hide') { S.ov.hidden.push(id); }
      else if (op === 'add') { S.ov.hidden.splice(S.ov.hidden.indexOf(id), 1); }
      saveOv(); renderOverview();
      const again = $('[data-ovw="' + op + '"][data-id="' + id + '"]', $('#ovBody')); if (again && !again.disabled) again.focus();
      return;
    }
    const a = e.target.closest('[data-act]');
    if (a) {
      const act = a.dataset.act;
      if (act === 'clear-ns') { S.ns.clear(); return commitNs(); }
      if (act === 'done') { S.ovEdit = false; return renderOverview(); }
      if (act === 'reset') { S.ov = {order:OV_ORDER.slice(), hidden:['stats'], size:{}, lane:{}, thr:3}; saveOv(); showToast('Overview reset'); return renderOverview(); }
      const thr = S.ov.thr;
      if (act === 'failing') return openKindFiltered('pods', 'Failing pods', r => FAILING.has(r.status));
      if (act === 'pending') return openKindFiltered('pods', 'Pending pods', r => r.status === 'Pending' || r.status === 'ContainerCreating');
      if (act === 'problems-all') return openKindFiltered('pods', 'Not running', r => PROBLEM.has(r.status));
      if (act === 'restarts') return openKindFiltered('pods', 'Restarts ≥ ' + thr, r => r.restarts >= thr);
      if (act === 'degraded') return openKindFiltered('deployments', 'Degraded', r => r.have < r.want);
      if (act === 'nodes') return openKindFiltered('nodes', 'Not ready', r => r.status !== 'Ready');
      if (act === 'argo') return openKindFiltered('argoapps', 'Needs sync', r => r.sync !== 'Synced' || r.health !== 'Healthy');
      if (act === 'warnings') { openKindFiltered('events', 'Warnings', r => r.type === 'Warning'); S.flt.evtype = 'Warning'; return renderChips(); }
    }
    const g = e.target.closest('[data-goto]'); if (g && !S.ovEdit) return gotoResource(g.dataset.goto);
    const n = e.target.closest('[data-ns-open]'); if (n && !S.ovEdit) return openKindInNs('pods', n.dataset.nsOpen);
    const f = e.target.closest('[data-evf]'); if (f) { S.evFilter = f.dataset.evf; return renderOverview(); }
    const k = e.target.closest('[data-kindgo]'); if (k && !S.ovEdit) setView('workloads', k.dataset.kindgo);
  });
  $('#ovBody').addEventListener('change', e => { if (e.target.matches('[data-ov-thr]')) { S.ov.thr = parseInt(e.target.value, 10); saveOv(); renderOverview(); } });
  $('#ovBody').addEventListener('keydown', e => { const r = e.target.closest('tr.go'); if (r && e.target === r && (e.key === 'Enter' || e.key === ' ')) { e.preventDefault(); gotoResource(r.dataset.goto); } });
  $('#ovCustomize').addEventListener('click', () => { S.ovEdit = !S.ovEdit; renderOverview(); });

  /* drag a widget by its edge handle or its title bar; arrow keys on the handle also move it */
  let ovDrag = null;
  const OV_SKIP = 'button,select,input,textarea,a,.range-tabs,.chip-row,.thr,.chart-svg,.ov-w-bar';
  const isIn = (r, x, y) => x >= r.left && x <= r.right && y >= r.top && y <= r.bottom;
  const narrowOv = () => window.innerWidth <= 1024;
  const kidsOf2 = (el, cls) => Array.from(el.children).filter(c => c.classList.contains(cls));
  const ovCards = root => Array.from(root.querySelectorAll('.ov-w')).filter(x => !ovDrag || x !== ovDrag.el);
  function ovFlip(fn){
    const kids = ovCards(ovDrag.box), before = new Map(kids.map(k => [k, k.getBoundingClientRect()]));
    fn();
    before.forEach((r, k) => {
      if (!k.animate) return;
      const n = k.getBoundingClientRect(), dx = r.left - n.left, dy = r.top - n.top;
      if (dx || dy) k.animate([{transform:'translate(' + dx + 'px,' + dy + 'px)'}, {transform:'none'}], {duration:240, easing:'cubic-bezier(.2,.8,.2,1)'});
    });
  }
  function ovNewBand(){
    const band = document.createElement('div');
    band.className = 'ov-band'; band.setAttribute('data-band', '');
    band.innerHTML = ['L', 'R'].map(k => '<div class="ov-lane" data-lane="' + k + '"><div class="ov-lane-end" aria-hidden="true"></div></div>').join('');
    return band;
  }
  function ovTidy(){
    const d = ovDrag, box = d.box;
    let b = box.firstElementChild;
    while (b) {   // two bands side by side are one band
      const nx = b.nextElementSibling;
      if (b.classList.contains('ov-band') && nx && nx.classList.contains('ov-band')) {
        kidsOf2(b, 'ov-lane').forEach((to, i) => { const from = kidsOf2(nx, 'ov-lane')[i], end = kidsOf2(to, 'ov-lane-end')[0]; kidsOf2(from, 'ov-w').forEach(c => to.insertBefore(c, end)); });
        nx.remove();
        continue;
      }
      b = nx;
    }
    kidsOf2(box, 'ov-band').forEach(band => { if (!ovCards(band).length) band.remove(); });
    const inLane = !!d.ph.closest('.ov-lane');
    d.ph.classList.toggle('half', inLane); d.ph.classList.toggle('full', !inLane);
  }
  function ovStart(){
    const d = ovDrag, el = d.el, box = el.closest('.ov-widgets'), rect = el.getBoundingClientRect();
    clearTimeout(d.timer);
    d.started = true; d.box = box; d.moved = 0; d.pref = el.dataset.pref || 'full';
    d.ph = document.createElement('div');
    d.ph.className = 'ov-w ov-ph ' + (el.closest('.ov-lane') ? 'half' : 'full');
    d.ph.style.minHeight = rect.height + 'px';
    el.parentNode.insertBefore(d.ph, el);
    box.appendChild(el);          // the lifted card is position:fixed, keep it out of the lanes
    el.style.width = rect.width + 'px'; el.style.height = rect.height + 'px';
    el.style.left = rect.left + 'px'; el.style.top = rect.top + 'px';
    el.classList.add('dragging');
    box.classList.add('is-dragging');
    document.body.classList.add('ov-dragging');
    if (navigator.vibrate) { try { navigator.vibrate(8); } catch (err) { /* unsupported */ } }
    const tick = () => {
      if (!ovDrag || !ovDrag.started) return;
      const mr = main.getBoundingClientRect(), y = ovDrag.ly, before = main.scrollTop;
      if (y < mr.top + 80) main.scrollTop -= Math.min(20, (mr.top + 80 - y) / 3);
      else if (y > mr.bottom - 80) main.scrollTop += Math.min(20, (y - (mr.bottom - 80)) / 3);
      if (main.scrollTop !== before) ovPlace(ovDrag.lx, ovDrag.ly);
      ovDrag.raf = requestAnimationFrame(tick);
    };
    d.raf = requestAnimationFrame(tick);
  }
  /* Where to insert inside a lane: compare the pointer with each card's midpoint. Cards below the placeholder
     switch at 40% and cards above at 60%, so a card never flips back as soon as it has moved. */
  function laneRef(lane, y){
    const d = ovDrag, cards = kidsOf2(lane, 'ov-w').filter(c => c !== d.el), phIdx = cards.indexOf(d.ph);
    for (let i = 0; i < cards.length; i++) {
      if (cards[i] === d.ph) continue;
      const r = cards[i].getBoundingClientRect();
      if (y < r.top + r.height * (phIdx >= 0 ? (i > phIdx ? 0.4 : 0.6) : 0.5)) return cards[i];
    }
    return kidsOf2(lane, 'ov-lane-end')[0] || null;
  }
  function ovPlace(x, y){
    const d = ovDrag, now = Date.now();
    if (!d || !d.started || now - d.moved < 110) return;
    if (isIn(d.ph.getBoundingClientRect(), x, y)) return;
    const box = d.box, tops = Array.from(box.children).filter(k => k !== d.el);
    let hit = tops.find(k => isIn(k.getBoundingClientRect(), x, y));
    if (!hit) { let best = Infinity; tops.forEach(k => { const r = k.getBoundingClientRect(), dy = Math.max(r.top - y, 0, y - r.bottom); if (dy < best) { best = dy; hit = k; } }); }
    if (!hit || hit === d.ph) return;
    const hr = hit.getBoundingClientRect();
    let place = null;
    if (hit.classList.contains('ov-band')) {
      const rel = (y - hr.top) / Math.max(1, hr.height), lanes = kidsOf2(hit, 'ov-lane');
      if (d.pref === 'full' && (rel < 0.2 || rel > 0.8)) {
        const ref = rel < 0.5 ? hit : hit.nextSibling;                      // a full-width card goes above / below the band
        place = () => box.insertBefore(d.ph, ref);
      } else if (narrowOv()) {                                             // single column: lanes are flattened
        const cs = ovCards(hit).filter(c => c !== d.ph);
        let t = cs.find(c => isIn(c.getBoundingClientRect(), x, y)), best = Infinity;
        if (!t) cs.forEach(c => { const r = c.getBoundingClientRect(), dy = Math.max(r.top - y, 0, y - r.bottom); if (dy < best) { best = dy; t = c; } });
        if (!t) return;
        const r = t.getBoundingClientRect(), ref = y < r.top + r.height / 2 ? t : t.nextSibling, lane = t.parentNode;
        if (ref === d.ph || ref === d.ph.nextSibling) return;
        place = () => lane.insertBefore(d.ph, ref);
      } else {
        const lane = lanes[x < hr.left + hr.width / 2 ? 0 : 1], ref = laneRef(lane, y);
        if (ref === d.ph || ref === d.ph.nextElementSibling) return;
        place = () => lane.insertBefore(d.ph, ref);
      }
    } else {                                                                // a full-width card
      const before = y < hr.top + hr.height / 2;
      if (d.pref === 'half') {
        const sib = before ? hit.previousElementSibling : hit.nextElementSibling;
        if (sib && sib !== d.el && sib.classList.contains('ov-band')) {   // join the neighbouring band
          const lane = kidsOf2(sib, 'ov-lane')[x < hr.left + hr.width / 2 ? 0 : 1];
          place = () => lane.insertBefore(d.ph, before ? kidsOf2(lane, 'ov-lane-end')[0] : (kidsOf2(lane, 'ov-w')[0] || kidsOf2(lane, 'ov-lane-end')[0]));
        } else {
          place = () => { const band = ovNewBand(); box.insertBefore(band, before ? hit : hit.nextSibling); const lane = kidsOf2(band, 'ov-lane')[x < hr.left + hr.width / 2 ? 0 : 1]; lane.insertBefore(d.ph, kidsOf2(lane, 'ov-lane-end')[0]); };
        }
      } else {
        const phAfter = !!(hit.compareDocumentPosition(d.ph) & 4);
        if (phAfter ? y > hr.top + hr.height * 0.6 : y < hr.top + hr.height * 0.4) return;
        const ref = phAfter ? hit : hit.nextSibling;
        if (ref === d.ph) return;
        place = () => box.insertBefore(d.ph, ref);
      }
    }
    ovFlip(() => { place(); ovTidy(); });
    d.moved = now;
  }
  function ovMoveDrag(e){
    const d = ovDrag;
    d.lx = e.clientX; d.ly = e.clientY;
    d.el.style.left = (e.clientX - d.ox) + 'px';
    d.el.style.top = (e.clientY - d.oy) + 'px';
    ovPlace(e.clientX, e.clientY);
  }
  function ovFinish(apply){
    const d = ovDrag;
    if (!d) return;
    clearTimeout(d.timer);
    cancelAnimationFrame(d.raf);
    if (!d.started) { ovDrag = null; return; }
    const done = () => {
      const groups = [];
      Array.from(d.box.children).forEach(k => {
        if (k === d.el) return;
        if (k.classList.contains('ov-band')) {
          const g = {type:'band', L:[], R:[]};
          kidsOf2(k, 'ov-lane').forEach(lane => kidsOf2(lane, 'ov-w').forEach(c => { if (c !== d.el) g[lane.dataset.lane].push(c === d.ph ? d.id : c.dataset.w); }));
          if (g.L.length || g.R.length) groups.push(g);
        } else groups.push({type:'full', id:k === d.ph ? d.id : k.dataset.w});
      });
      ovDrag = null;
      document.body.classList.remove('ov-dragging');
      if (apply) ovCommit(groups);
      renderOverview();
      const g = $('[data-grip="' + d.id + '"]', $('#ovBody')); if (g) g.focus();
    };
    const pr = d.ph.getBoundingClientRect();
    d.el.classList.add('dropping');
    if (apply && d.el.animate) {
      const a = d.el.animate([{left:d.el.style.left, top:d.el.style.top}, {left:pr.left + 'px', top:pr.top + 'px'}], {duration:170, easing:'cubic-bezier(.2,.8,.2,1)'});
      a.onfinish = done;
    } else done();
  }
  $('#ovBody').addEventListener('pointerdown', e => {
    if (e.button > 0 || ovDrag || S.sr.open || (pull && pull.moved)) return;
    const grip = e.target.closest('.ov-grip'), head = grip ? null : e.target.closest('.ov-w .panel-head');
    if (!grip && (!head || e.target.closest(OV_SKIP))) return;
    const el = (grip || head).closest('.ov-w'), rect = el.getBoundingClientRect();
    ovDrag = {el, id:el.dataset.w, pid:e.pointerId, ox:e.clientX - rect.left, oy:e.clientY - rect.top, sx:e.clientX, sy:e.clientY, lx:e.clientX, ly:e.clientY, started:false, mode:grip ? 'grip' : 'head', touch:e.pointerType === 'touch'};
    try { el.setPointerCapture(e.pointerId); } catch (err) { /* not capturable */ }
    if (grip) e.preventDefault();
    else if (ovDrag.touch) ovDrag.timer = setTimeout(() => { if (ovDrag && !ovDrag.started) { ovStart(); ovMoveDrag({clientX:ovDrag.lx, clientY:ovDrag.ly}); } }, 260);
  });
  document.addEventListener('pointermove', e => {
    if (!ovDrag || e.pointerId !== ovDrag.pid) return;
    const d = ovDrag;
    if (!d.started) {
      const moved = Math.hypot(e.clientX - d.sx, e.clientY - d.sy);
      if (d.mode === 'head' && d.touch) { d.lx = e.clientX; d.ly = e.clientY; if (moved > 9) { clearTimeout(d.timer); ovDrag = null; } return; }
      if (moved < (d.mode === 'grip' ? 3 : 6)) return;
      ovStart();
    }
    ovMoveDrag(e);
  });
  document.addEventListener('pointerup', e => { if (ovDrag && e.pointerId === ovDrag.pid) ovFinish(true); });
  document.addEventListener('pointercancel', e => { if (ovDrag && e.pointerId === ovDrag.pid) ovFinish(false); });
  document.addEventListener('touchmove', e => { if (ovDrag && ovDrag.started) e.preventDefault(); }, {passive:false});
  document.addEventListener('keydown', e => { if (e.key === 'Escape' && ovDrag && ovDrag.started) ovFinish(false); });
  $('#ovBody').addEventListener('keydown', e => {
    const g = e.target.closest('.ov-grip');
    if (!g) return;
    const k = e.key;
    if (['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight'].indexOf(k) < 0) return;
    e.preventDefault();
    if (k === 'ArrowUp') ovMove(g.dataset.grip, -1); else if (k === 'ArrowDown') ovMove(g.dataset.grip, 1); else ovLane(g.dataset.grip);
    renderOverview();
    const again = $('[data-grip="' + g.dataset.grip + '"]', $('#ovBody')); if (again) again.focus();
  });


  function firstPrefix(r){
    if (!TREE_KINDS.has(S.kind)) return '';
    const has = kidsOf(S.kind, r).length > 0, open = S.open.has(r.id);
    return has ? '<button type="button" class="tree-toggle" data-ptoggle="' + esc(r.id) + '" aria-expanded="' + open + '" aria-label="' + (open ? 'Collapse ' : 'Expand ') + esc(r.name) + '">' + ICON('chevron') + '</button>' : '<span class="tree-spacer"></span>';
  }

  /* ------------------------------------------- global namespace filter */
  try { JSON.parse(lsGet('openkite-ns', '[]')).forEach(n => { if (DATA.namespaces.some(x => x.name === n)) S.ns.add(n); }); } catch (err) { /* ignore bad data */ }
  const nsMenu = $('#nsMenu'), nsPill = $('#nsPill');
  const inNs = r => !S.ns.size || !r.ns || S.ns.has(r.ns);
  const NSD = k => DATA[k].filter(inNs);
  function updateNsPill(){
    const n = S.ns.size, names = Array.from(S.ns).sort();
    nsPill.classList.toggle('active', n > 0);
    $('#nsPillText').textContent = n ? (n === 1 ? names[0] : names[0] + ' +' + (n - 1)) : 'Namespaces';
    $('#nsPillBadge').innerHTML = n ? cnt(n, 'on sel-count') : '<span class="all-badge"><span class="ab-word">All</span>' + cnt(DATA.namespaces.length) + '</span>';
    $('#nsPillClear').hidden = !n;
    $('#nsPillBtn').setAttribute('aria-label', n ? 'Namespace filter: ' + names.join(', ') + '. Change' : 'Namespace filter: all ' + DATA.namespaces.length + ' namespaces. Change');
    $('#nsPillBtn').title = n ? 'Namespaces: ' + names.slice(0, 4).join(', ') + (n > 4 ? ' +' + (n - 4) : '') : 'Filter by namespace';
  }
  function commitNs(){
    lsSet('openkite-ns', JSON.stringify(Array.from(S.ns)));
    updateNsPill(); updateCounts();
    if (S.view === 'workloads') { renderChips(); renderRows(); }
    if (S.view === 'overview') renderOverview();
    if (S.view === 'argocd') renderArgo();
  }
  S.nsq = '';
  // any namespace name shown in a details pane filters the whole console by it; clicking it again clears the filter
  document.addEventListener('click', e => {
    const b = e.target.closest && e.target.closest('[data-ns-filter]');
    if (!b) return;
    e.preventDefault();
    const ns = b.dataset.nsFilter, only = S.ns.size === 1 && S.ns.has(ns);
    S.ns = only ? new Set() : new Set([ns]);
    commitNs();
    showToast(only ? 'Showing all namespaces' : 'Filtering by namespace ' + ns);
  });
  function nsCounts(){
    const out = {};
    (KINDS[S.kind].cluster ? DATA.pods : rowsOf()).forEach(x => { if (x.ns) out[x.ns] = (out[x.ns] || 0) + 1; });
    return out;
  }
  function nsListHtml(){
    const q = S.nsq.trim().toLowerCase(), counts = nsCounts();
    const names = DATA.namespaces.map(n => n.name).sort((x, y) => x.localeCompare(y, undefined, {sensitivity:'base'})).filter(n => !q || n.toLowerCase().indexOf(q) >= 0);
    return names.length ? names.map(n => '<button type="button" class="rm-item nf-item" role="menuitemcheckbox" aria-checked="' + S.ns.has(n) + '" data-nsm="' + esc(n) + '"><span class="nf-box"></span><span class="rm-text">' + esc(n) + '</span>' + cnt(counts[n] || 0) + '</button>').join('')
      : '<p class="muted-note nsm-empty">No namespace matches “' + esc(S.nsq.trim()) + '”.</p>';
  }
  function nsMenuHtml(){
    return '<div class="nsm-head"><label class="search-field">' + ICON('search') + '<input type="search" id="nsmSearch" placeholder="Search namespaces…" aria-label="Search namespaces" autocomplete="off" value="' + esc(S.nsq) + '"></label></div>' +
      '<div class="nsm-actions"><span class="nsm-count" id="nsmCount"></span><button type="button" class="x-mini" data-nsnone aria-label="Clear selection" title="Clear selection">' + ICON('close') + '</button></div>' +
      '<div class="nsm-list" id="nsmList" role="menu" aria-label="Namespaces">' + nsListHtml() + '</div><div class="nsm-foot">Applies to every page</div>';
  }
  function refreshNsMenu(){
    const list = $('#nsmList', nsMenu); if (!list) return;
    const st = list.scrollTop;
    list.innerHTML = nsListHtml(); list.scrollTop = st;
    $('#nsmCount', nsMenu).textContent = S.ns.size ? S.ns.size + ' of ' + DATA.namespaces.length + ' selected' : 'All namespaces';
  }
  function openNsMenu(anchor){
    closePops();
    S.nsq = '';
    nsMenu.innerHTML = nsMenuHtml();
    refreshNsMenu();
    placePop(nsMenu, anchor, false);
    anchor.setAttribute('aria-expanded', 'true');
    const i = $('#nsmSearch', nsMenu); if (i && window.matchMedia && window.matchMedia('(hover:hover)').matches) i.focus();
  }
  $('#nsPillBtn').addEventListener('click', e => { if (!nsMenu.hidden) return void closePops(); openNsMenu(e.currentTarget); });
  $('#nsPillClear').addEventListener('click', () => { S.ns.clear(); commitNs(); showToast('Showing all namespaces'); });
  nsMenu.addEventListener('input', e => { if (e.target.id === 'nsmSearch') { S.nsq = e.target.value; refreshNsMenu(); } });
  nsMenu.addEventListener('click', e => {
    const it = e.target.closest('[data-nsm]');
    if (it) { if (S.ns.has(it.dataset.nsm)) S.ns.delete(it.dataset.nsm); else S.ns.add(it.dataset.nsm); }
    else if (e.target.closest('[data-nsnone]')) S.ns.clear();
    else return;
    commitNs(); refreshNsMenu();
    $('#nsPillBtn').setAttribute('aria-expanded', 'true');
  });

  /* ---------------------------------------------- pull-down global search */
  const sheet = $('#searchSheet'), sInput = $('#sheetInput'), sResults = $('#sheetResults'), grip = $('#searchOrb'), zone = $('#pullZone');
  S.sr = {scope:'all', idx:0, items:[], open:false};
  const SCOPES = [['all', 'All'], ['menu', 'Menu'], ['res', 'Resources'], ['cluster', 'Clusters'], ['action', 'Actions']];
  function searchIndex(){
    const out = [];
    NAV.forEach(sec => sec.items.forEach(it => {
      if (it[0] === 'soon') return;
      const isKind = it[0] === 'kind';
      out.push({type:'menu', label:itemLabel(it), sub:sec.g, icon:isKind ? KINDS[it[1]].icon : it[3], run:() => setView(isKind ? 'workloads' : it[1], isKind ? it[1] : undefined)});
    }));
    Object.keys(DATA).forEach(k => DATA[k].forEach(r => out.push({type:'res', label:r.name, sub:KINDS[k].one + (r.ns ? ' · ' + r.ns : ''), icon:KINDS[k].icon, run:() => gotoResource(r.id)})));
    CLUSTERS.forEach(c => out.push({type:'cluster', label:c.id, sub:'Switch cluster · ' + c.ver + ' · ' + c.region, icon:'cluster', run:() => { S.cluster = c.id; lsSet('openkite-cluster', c.id); applyCluster(); showToast('Switched to ' + c.id); }}));
    [['Toggle dark mode', 'moon', () => { const n = document.documentElement.dataset.theme === 'dark' ? 'light' : 'dark'; applyTheme(n, true); showToast(n === 'dark' ? 'Dark mode on' : 'Light mode on'); }],
     ['Refresh resources', 'refresh', refreshAll], ['Open settings', 'settings', openSettings], ['Customize overview', 'filter', () => { S.ovEdit = true; setView('overview'); }],
     ['Menu layout: tree', 'tree', () => setNavLayout('tree')], ['Menu layout: tree-grid', 'treegrid', () => setNavLayout('grid')],
     ['Show or hide logs & terminal', 'dock', () => setDock(!isOpen())], ['New terminal', 'terminal', () => { const t = makeTerminal(); setDock(true); showTab(t.id); }],
     ['Add kubeconfig directory…', 'plus', openKubeDialog], ['Clear namespace filter', 'close', () => { S.ns.clear(); commitNs(); }], ['Keyboard shortcuts', 'terminal', openShortcuts]]
      .forEach(a => out.push({type:'action', label:a[0], sub:'Action', icon:a[1], run:a[2]}));
    return out;
  }
  function scoreOf(q, s){
    s = s.toLowerCase();
    const i = s.indexOf(q);
    if (i === 0) return 100;
    if (i > 0) return 80 - Math.min(i, 30);
    if (q.length < 3) return 0;
    let j = 0, first = -1, last = -1;
    for (let k = 0; k < s.length && j < q.length; k++) if (s[k] === q[j]) { if (first < 0) first = k; last = k; j++; }
    return j === q.length && last - first < q.length * 2 ? 30 : 0;
  }
  const hl = (s, q) => { const i = q ? s.toLowerCase().indexOf(q) : -1; return i < 0 ? esc(s) : esc(s.slice(0, i)) + '<mark>' + esc(s.slice(i, i + q.length)) + '</mark>' + esc(s.slice(i + q.length)); };
  const GROUP_NAME = {menu:'Menu', res:'Resources', cluster:'Clusters', action:'Actions'};
  function runSearch(){
    const q = sInput.value.trim().toLowerCase(), sc = S.sr.scope, idx = searchIndex();
    const pool = idx.filter(x => sc === 'all' || x.type === sc);
    const limit = sc === 'all' ? 5 : 40;
    let groups;
    if (!q) groups = (sc === 'all' ? ['action', 'menu'] : [sc]).map(t => [t, pool.filter(x => x.type === t).slice(0, sc === 'all' ? 5 : 40)]);
    else groups = ['menu', 'res', 'cluster', 'action'].filter(t => sc === 'all' || sc === t).map(t => { const sc2 = pool.filter(x => x.type === t).map(x => ({x, s:Math.max(scoreOf(q, x.label), scoreOf(q, x.sub) * 0.6)})).filter(y => y.s > 0).sort((a, b) => b.s - a.s); return [t, sc2.slice(0, limit).map(y => y.x), sc2.length ? sc2[0].s : 0]; }).sort((a, b) => b[2] - a[2]);
    groups = groups.filter(g => g[1].length);
    S.sr.items = [];
    let html = groups.map(g => '<div class="sr-group">' + (q || sc !== 'all' ? GROUP_NAME[g[0]] : g[0] === 'action' ? 'Quick actions' : 'Jump to') + '</div>' + g[1].map(x => {
      const i = S.sr.items.push(x) - 1;
      return '<button type="button" class="sr-row" role="option" id="sr' + i + '" data-i="' + i + '" aria-selected="false">' + ICON(x.icon) + '<span class="sr-main"><span class="sr-l">' + hl(x.label, q) + '</span><span class="sr-s">' + esc(x.sub) + '</span></span><kbd>↵</kbd></button>';
    }).join('')).join('');
    if (!html) html = '<p class="sr-empty">Nothing matches “' + esc(sInput.value.trim()) + '”. Try a resource name, a page, a cluster or an action.</p>';
    sResults.innerHTML = html;
    S.sr.idx = 0;
    markRow();
  }
  function markRow(){
    $$('.sr-row', sResults).forEach((r, i) => r.setAttribute('aria-selected', String(i === S.sr.idx)));
    const cur = $('#sr' + S.sr.idx);
    if (cur) {
      sInput.setAttribute('aria-activedescendant', 'sr' + S.sr.idx);
      const top = cur.offsetTop - 40, bot = cur.offsetTop + cur.offsetHeight + 8;
      if (top < sResults.scrollTop) sResults.scrollTop = Math.max(0, top); else if (bot > sResults.scrollTop + sResults.clientHeight) sResults.scrollTop = bot - sResults.clientHeight;
    }
  }
  function renderScopes(){
    $('#sheetScopes').innerHTML = SCOPES.map(s => '<button type="button" class="chip' + (S.sr.scope === s[0] ? ' active' : '') + '" data-scope="' + s[0] + '" aria-pressed="' + (S.sr.scope === s[0]) + '">' + s[1] + '</button>').join('');
  }
  function setSheet(p, animate){
    sheet.style.transition = animate ? '' : 'none';
    sheet.style.transform = 'translateY(' + (-(1 - p) * 100) + '%)';
    grip.style.setProperty('--pull', p);
  }
  function refreshAll(){
    showToast('Resources refreshed');
    if (S.view === 'overview') renderOverview(); else if (S.view === 'workloads') renderWorkloads();
  }
  function openSearch(prefill){
    closePops();
    S.sr.open = true;
    sheet.classList.add('open'); sheet.setAttribute('aria-hidden', 'false'); grip.setAttribute('aria-expanded', 'true');
    document.body.classList.add('searching');
    setSheet(1, true);
    sheet.style.transform = '';
    if (typeof prefill === 'string') sInput.value = prefill;
    renderScopes(); runSearch();
    setTimeout(() => { sInput.focus(); sInput.select(); }, 30);
  }
  function closeSearch(){
    S.sr.open = false;
    sheet.classList.remove('open', 'pulling'); sheet.setAttribute('aria-hidden', 'true'); grip.setAttribute('aria-expanded', 'false');
    document.body.classList.remove('searching');
    sheet.style.transition = ''; sheet.style.transform = ''; grip.style.setProperty('--pull', 0);
    sInput.blur();
  }
  function pickResult(i){
    const x = S.sr.items[i];
    if (!x) return;
    closeSearch();
    setTimeout(() => { navClear(); x.run(); }, 60);
  }
  sInput.addEventListener('input', runSearch);
  sInput.addEventListener('keydown', e => {
    const n = S.sr.items.length;
    if (e.key === 'ArrowDown') { e.preventDefault(); if (n) { S.sr.idx = (S.sr.idx + 1) % n; markRow(); } }
    else if (e.key === 'ArrowUp') { e.preventDefault(); if (n) { S.sr.idx = (S.sr.idx - 1 + n) % n; markRow(); } }
    else if (e.key === 'Enter') { e.preventDefault(); pickResult(S.sr.idx); }
    else if (e.key === 'Tab') { e.preventDefault(); const i = SCOPES.findIndex(s => s[0] === S.sr.scope); S.sr.scope = SCOPES[(i + (e.shiftKey ? SCOPES.length - 1 : 1)) % SCOPES.length][0]; renderScopes(); runSearch(); }
  });
  sResults.addEventListener('click', e => { const r = e.target.closest('.sr-row'); if (r) pickResult(Number(r.dataset.i)); });
  sResults.addEventListener('pointermove', e => { const r = e.target.closest('.sr-row'); if (r && Number(r.dataset.i) !== S.sr.idx) { S.sr.idx = Number(r.dataset.i); $$('.sr-row', sResults).forEach((x, i) => x.setAttribute('aria-selected', String(i === S.sr.idx))); } });
  $('#sheetScopes').addEventListener('click', e => { const b = e.target.closest('[data-scope]'); if (b) { S.sr.scope = b.dataset.scope; renderScopes(); runSearch(); sInput.focus(); } });
  $('#sheetClear').addEventListener('click', () => { sInput.value = ''; runSearch(); sInput.focus(); });
  $('#sheetBackdrop').addEventListener('click', closeSearch);
  $('#sheetGrip').addEventListener('click', () => { if (S.sr.open) closeSearch(); });   // keyboard / assistive activation
  document.addEventListener('keydown', e => {
    const typing = /^(INPUT|TEXTAREA|SELECT)$/.test((e.target.tagName || '')) || e.target.isContentEditable;
    if ((e.key === 'k' || e.key === 'K') && (e.ctrlKey || e.metaKey)) { e.preventDefault(); if (S.sr.open) closeSearch(); else openSearch(''); }
    else if (e.key === '/' && !typing && !e.ctrlKey && !e.metaKey && dlgScrim.hidden && !ovDrag) { e.preventDefault(); openSearch(''); }
  });

  /* drag the top-edge tab down to reveal the sheet; drag the sheet's lower edge up to dismiss */
  let pull = null;
  function pullStart(e, mode){
    if (ovDrag || (typeof drag !== 'undefined' && drag && drag.started)) return;
    if (!S.sr.open && mode === 'close') return;
    pull = {mode, sy:e.clientY, pid:e.pointerId, moved:false, h:Math.max(sheet.offsetHeight, 320)};
  }
  function pullMove(e){
    if (!pull || e.pointerId !== pull.pid) return;
    const dy = e.clientY - pull.sy;
    if (!pull.moved && Math.abs(dy) < 5) return;
    pull.moved = true;
    if (pull.mode === 'open' && !S.sr.open) {
      if (!sheet.classList.contains('pulling')) { sheet.classList.add('pulling'); document.body.classList.add('searching'); renderScopes(); runSearch(); }
      pull.p = Math.max(0, Math.min(1, dy / (pull.h * 0.6)));
      setSheet(pull.p, false);
    } else if (pull.mode === 'close') {
      pull.p = Math.max(0, Math.min(1, 1 + dy / (pull.h * 0.6)));
      setSheet(pull.p, false);
    }
  }
  function pullEnd(e){
    if (!pull || e.pointerId !== pull.pid) return;
    const p = pull;
    pull = null;
    sheet.style.transition = '';
    if (!p.moved) { if (p.mode === 'open') { if (S.sr.open) closeSearch(); else openSearch(''); } else closeSearch(); return; }
    if (p.mode === 'open') { sheet.classList.remove('pulling'); if (p.p > 0.38) openSearch(''); else closeSearch(); }
    else if (p.p < 0.62) closeSearch(); else { sheet.style.transform = ''; }
    grip.style.setProperty('--pull', S.sr.open ? 1 : 0);
  }
  [grip, zone].forEach(el => el.addEventListener('pointerdown', e => { if (e.button > 0) return; e.preventDefault(); pullStart(e, 'open'); }));
  $('#sheetGrip').addEventListener('pointerdown', e => { if (e.button > 0) return; e.preventDefault(); pullStart(e, 'close'); });
  document.addEventListener('pointermove', pullMove);
  document.addEventListener('pointerup', pullEnd);
  document.addEventListener('pointercancel', e => { if (pull && e.pointerId === pull.pid) { pull = null; if (!S.sr.open) closeSearch(); else { sheet.style.transition = ''; sheet.style.transform = ''; } } });
  document.addEventListener('touchmove', e => { if (pull && pull.moved) e.preventDefault(); }, {passive:false});

  grip.addEventListener('keydown', e => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); if (S.sr.open) closeSearch(); else openSearch(''); } });

  /* --------------------------------------------------- sidebar + menu + theme */
  const navTree = $('#navTree');
  function navItem(it, cls, active){
    if (it[0] === 'soon') return '<button class="' + cls + '" type="button" data-toast="' + it[1] + ' is part of the full OpenKite product">' + ICON(it[2]) + '<span>' + it[1] + '</span></button>';
    const isKind = it[0] === 'kind', kind = isKind ? it[1] : null, K = isKind ? KINDS[kind] : null;
    const label = isKind ? K.label : it[2], icon = isKind ? K.icon : it[3];
    const on = isKind ? (active.view === 'workloads' && active.kind === kind) : active.view === it[1];
    const badge = isKind && cls === 'nav-item' ? '<span class="nav-badge" data-count="' + kind + '">' + DATA[kind].length + '</span>' : '';
    return '<button class="' + cls + (on ? ' active' : '') + '" type="button" data-view="' + (isKind ? 'workloads' : it[1]) + '"' + (isKind ? ' data-kind="' + kind + '"' : '') + (on ? ' aria-current="page"' : '') + '>' + ICON(icon) + '<span>' + esc(label) + '</span>' + badge + '</button>';
  }
  /* ---- menu layout (tree / tabs), category filter and text filter */
  S.navLayout = lsGet('openkite-navlayout', 'grid') === 'tree' ? 'tree' : 'grid';
  try { const g = JSON.parse(lsGet('openkite-navgroups', 'null')); S.navGroups = Array.isArray(g) ? g.filter(x => NAV.some(s => s.g === x)) : null; } catch (err) { S.navGroups = null; }
  if (S.navGroups && !S.navGroups.length) S.navGroups = null;
  S.navQ = ''; S.navTab = 'Workloads';
  const SHORT_G = {'Access Control':'Access', 'Custom Resources':'Custom'};
  const itemLabel = it => it[0] === 'soon' ? it[1] : it[0] === 'kind' ? KINDS[it[1]].label : it[2];
  const curGroup = () => S.view === 'overview' ? 'Cluster' : S.view === 'argocd' ? 'Argo CD' : KINDS[S.kind].group;
  function navSection(sec, items, open, active){
    return '<div class="nav-section' + (sec.argo ? ' argo' : '') + '"><button class="nav-group" type="button" data-group="' + esc(sec.g) + '" aria-expanded="' + open + '"><span>' + esc(sec.g) + '</span>' + ICON('chevron') + '</button>' +
      '<div class="nav-items"' + (open ? '' : ' hidden') + '>' + items.map(it => navItem(it, 'nav-item', active)).join('') + '</div></div>';
  }
  const TILE = {pvcs:'PVCs', pvs:'PVs', storageclasses:'Classes', networkpolicies:'NetPolicies', serviceaccounts:'Accounts', clusterroles:'ClusterRoles', rolebindings:'Bindings', servicemonitors:'Monitors', certificates:'Certs'};
  function gridTile(it, active){
    if (it[0] === 'soon') return '<button class="nav-tile" type="button" data-toast="' + esc(it[1]) + ' is part of the full OpenKite product">' + ICON(it[2]) + '<span class="nt-l">' + esc(it[1]) + '</span></button>';
    const isKind = it[0] === 'kind', label = itemLabel(it), icon = isKind ? KINDS[it[1]].icon : it[3];
    const on = isKind ? active.view === 'workloads' && active.kind === it[1] : active.view === it[1];
    return '<button class="nav-tile' + (on ? ' active' : '') + '" type="button" data-view="' + (isKind ? 'workloads' : it[1]) + '"' + (isKind ? ' data-kind="' + it[1] + '"' : '') + (on ? ' aria-current="page"' : '') + ' title="' + esc(label) + '">' + ICON(icon) + '<span class="nt-l">' + esc(isKind && TILE[it[1]] || label) + '</span>' + (isKind ? '<span class="nav-badge" data-count="' + it[1] + '">' + DATA[it[1]].length + '</span>' : '') + '</button>';
  }
  function navTreeHtml(active){
    const secs = NAV.filter(s => !S.navGroups || S.navGroups.indexOf(s.g) >= 0);
    if (!secs.length) return '<p class="muted-note nav-empty">No categories selected.</p>';
    const grid = S.navLayout === 'grid';
    return secs.map(sec => {
      const open = navOpen.has(sec.g);
      return '<div class="nav-section' + (sec.argo ? ' argo' : '') + '"><button class="nav-group" type="button" data-group="' + esc(sec.g) + '" aria-expanded="' + open + '"><span>' + esc(sec.g) + '</span>' + ICON('chevron') + '</button>' +
        (grid ? '<div class="nav-grid"' + (open ? '' : ' hidden') + '>' + sec.items.map(it => gridTile(it, active)).join('') + '</div>'
          : '<div class="nav-items"' + (open ? '' : ' hidden') + '>' + sec.items.map(it => navItem(it, 'nav-item', active)).join('') + '</div>') + '</div>';
    }).join('');
  }
  function updateNavControls(){
    const n = S.navGroups ? S.navGroups.length : NAV.length;
    $('#navFilterLabel').textContent = S.navGroups && n < NAV.length ? n + ' of ' + NAV.length + ' categories' : 'All categories';
    $('#navFilterBtn').classList.toggle('on', !!S.navGroups);
    $$('[data-layout]').forEach(b => b.setAttribute('aria-pressed', String(b.dataset.layout === S.navLayout)));
  }
  function setNavLayout(v){
    S.navLayout = v === 'grid' ? 'grid' : 'tree'; lsSet('openkite-navlayout', S.navLayout);
    updateNavControls();
  updateNsPill(); renderNav();
  }
  function renderSubTabs(){ const el = $('#subTabs'); if (el) el.hidden = true; }
  $('#navControls').addEventListener('click', e => { const b = e.target.closest('[data-layout]'); if (b) setNavLayout(b.dataset.layout); });
  const navFilterMenu = $('#navFilterMenu'), crumbMenu = $('#crumbMenu');
  function navFilterHtml(){
    const on = g => !S.navGroups || S.navGroups.indexOf(g) >= 0;
    return '<div class="menu-group">Show categories</div><button type="button" class="rm-item nf-item" role="menuitemcheckbox" aria-checked="' + !S.navGroups + '" data-ngroup="*"><span class="nf-box"></span><span class="rm-text">All categories</span></button>' +
      NAV.map(s => '<button type="button" class="rm-item nf-item" role="menuitemcheckbox" aria-checked="' + on(s.g) + '" data-ngroup="' + esc(s.g) + '"><span class="nf-box"></span><span class="rm-text">' + esc(s.g) + '</span><small>' + s.items.length + '</small></button>').join('');
  }
  $('#navFilterBtn').addEventListener('click', e => {
    const b = e.currentTarget;
    if (!navFilterMenu.hidden) return void closePops();
    closePops(); navFilterMenu.innerHTML = navFilterHtml(); placePop(navFilterMenu, b, false); b.setAttribute('aria-expanded', 'true');
  });
  navFilterMenu.addEventListener('click', e => {
    const it = e.target.closest('[data-ngroup]');
    if (!it) return;
    const g = it.dataset.ngroup;
    if (g === '*') S.navGroups = null;
    else {
      const cur = S.navGroups ? S.navGroups.slice() : NAV.map(s => s.g), i = cur.indexOf(g);
      if (i >= 0) { if (cur.length > 1) cur.splice(i, 1); } else cur.push(g);
      S.navGroups = cur.length === NAV.length ? null : NAV.map(s => s.g).filter(x => cur.indexOf(x) >= 0);
    }
    lsSet('openkite-navgroups', JSON.stringify(S.navGroups));
    navFilterMenu.innerHTML = navFilterHtml();
    updateNavControls(); renderNav();
  });

  /* ---- breadcrumb dropdowns */
  function crumbItem(it, checked){
    if (it[0] === 'soon') return '<button type="button" class="rm-item" role="menuitem" data-toast="' + esc(it[1]) + ' is part of the full OpenKite product">' + ICON(it[2]) + '<span class="rm-text">' + esc(it[1]) + '</span></button>';
    const isKind = it[0] === 'kind', label = itemLabel(it), icon = isKind ? KINDS[it[1]].icon : it[3];
    return '<button type="button" class="rm-item" role="menuitemradio" aria-checked="' + checked + '" data-view="' + (isKind ? 'workloads' : it[1]) + '"' + (isKind ? ' data-kind="' + it[1] + '"' : '') + '>' + ICON(icon) + '<span class="rm-text">' + esc(label) + '</span>' + (isKind ? '<small>' + DATA[it[1]].length + '</small>' : '') + (checked ? ICON('check') : '') + '</button>';
  }
  function openCrumbMenu(anchor, mode){
    closePops();
    const g = curGroup();
    if (mode === 'section') crumbMenu.innerHTML = '<div class="menu-group">Category</div>' + NAV.map(s => crumbItem(s.items[0], s.g === g).replace(/<span class="rm-text">[^<]*<\/span>(<small>[^<]*<\/small>)?/, '<span class="rm-text">' + esc(s.g) + '</span>')).join('');
    else { const sec = NAV.find(s => s.g === g); crumbMenu.innerHTML = '<div class="menu-group">' + esc(g) + '</div>' + sec.items.map(it => crumbItem(it, it[0] === 'kind' ? S.view === 'workloads' && it[1] === S.kind : S.view === it[1])).join(''); }
    placePop(crumbMenu, anchor, false);
    anchor.setAttribute('aria-expanded', 'true');
    const f = $('[aria-checked="true"]', crumbMenu) || $('.rm-item', crumbMenu); if (f) f.focus();
  }
  $('#crumbSectionBtn').addEventListener('click', e => { if (!crumbMenu.hidden && e.currentTarget.getAttribute('aria-expanded') === 'true') closePops(); else openCrumbMenu(e.currentTarget, 'section'); });
  $('#crumbCurrentBtn').addEventListener('click', e => { if (!crumbMenu.hidden && e.currentTarget.getAttribute('aria-expanded') === 'true') closePops(); else openCrumbMenu(e.currentTarget, 'current'); });
  crumbMenu.addEventListener('click', e => { const t = e.target.closest('[data-toast]'); if (t) { closePops(); showToast(t.dataset.toast); } });

  function menuNavHtml(active){
    const grid = S.navLayout === 'grid', pr = v => String(S.navLayout === v);
    const seg = '<div class="menu-layout"><span>Layout</span><div class="seg-icons" role="group" aria-label="Menu layout"><button class="seg-ic" type="button" data-layout="tree" aria-pressed="' + pr('tree') + '" aria-label="Tree layout" title="Tree">' + ICON('tree') + '</button><button class="seg-ic" type="button" data-layout="grid" aria-pressed="' + pr('grid') + '" aria-label="Tree-grid layout" title="Tree-grid">' + ICON('treegrid') + '</button></div></div>';
    return seg + NAV.map(sec => {
      const open = navOpen.has(sec.g);
      return '<div class="menu-sec' + (sec.argo ? ' argo' : '') + '"><button class="nav-group menu-gb" type="button" data-group="' + esc(sec.g) + '" aria-expanded="' + open + '"><span>' + esc(sec.g) + '</span>' + ICON('chevron') + '</button>' +
        (grid ? '<div class="menu-grid"' + (open ? '' : ' hidden') + '>' + sec.items.map(it => gridTile(it, active)).join('') + '</div>'
          : '<div class="menu-items"' + (open ? '' : ' hidden') + '>' + sec.items.map(it => navItem(it, 'menu-item', active)).join('') + '</div>') + '</div>';
    }).join('');
  }
  function toggleNavGroup(name){
    if (navOpen.has(name)) navOpen.delete(name);
    else {
      if (NAV_PINNED.indexOf(name) < 0) Array.from(navOpen).forEach(x => { if (NAV_PINNED.indexOf(x) < 0) navOpen.delete(x); });   // accordion for the rest
      navOpen.add(name);
    }
  }
  /* swipe the side pane out to the right to dismiss it. Touch and pen can start anywhere that is not itself a horizontal scroller or a control;
     a mouse drags it by the header. Once it moves it follows the finger, then either flies out or springs back. */
  let idrag = null;
  const IDRAG_SKIP = 'button,a,input,textarea,select,label,pre.yaml,.yaml-ed,.table-wrap,.table-scroll,.chart-wrap,.chart-svg,[contenteditable]';
  const resetInspStyle = () => { inspector.style.transition = ''; inspector.style.transform = ''; inspectorScrim.style.transition = ''; inspectorScrim.style.opacity = ''; };
  inspector.addEventListener('pointerdown', e => {
    if (!inspector.classList.contains('open') || e.isPrimary === false || (e.pointerType === 'mouse' && e.button > 0)) return;
    const onHandle = !!e.target.closest('.insp-handle'), inHead = !!e.target.closest('.inspector-header');
    if (e.pointerType === 'mouse' && !inHead && !onHandle) return;
    if (e.target.closest(IDRAG_SKIP) && !onHandle && !(inHead && !e.target.closest('button,a,input'))) return;
    idrag = {x:e.clientX, y:e.clientY, pid:e.pointerId, t:Date.now(), active:false, dx:0, w:Math.max(inspector.offsetWidth, 300)};
  });
  inspector.addEventListener('pointermove', e => {
    if (!idrag || e.pointerId !== idrag.pid) return;
    const dx = e.clientX - idrag.x, dy = e.clientY - idrag.y;
    if (!idrag.active) {
      if (Math.abs(dy) > 12 && Math.abs(dy) > Math.abs(dx)) { idrag = null; return; }   // vertical: let the pane scroll
      if (dx < 10 || Math.abs(dx) < Math.abs(dy) * 1.3) return;
      idrag.active = true; inspector.style.transition = 'none'; inspectorScrim.style.transition = 'none';
      try { inspector.setPointerCapture(e.pointerId); } catch (err) { /* not supported */ }
    }
    idrag.dx = Math.max(0, dx);
    inspector.style.transform = 'translateX(' + idrag.dx + 'px)';
    inspectorScrim.style.opacity = String(Math.max(0, 1 - idrag.dx / idrag.w));
    if (e.cancelable) e.preventDefault();
  });
  const inspPointerEnd = e => {
    const d = idrag;
    if (!d || (e && e.pointerId !== d.pid)) return;
    idrag = null;
    if (!d.active) return;
    inspector.style.transition = 'transform 0.2s ease'; inspectorScrim.style.transition = 'opacity 0.2s ease';
    if (d.dx > d.w * 0.3 || (Date.now() - d.t < 280 && d.dx > 50)) {
      inspector.style.transform = 'translateX(105%)'; inspectorScrim.style.opacity = '0';
      setTimeout(() => { closeInspector(); resetInspStyle(); }, 200);
    } else { inspector.style.transform = ''; inspectorScrim.style.opacity = ''; setTimeout(resetInspStyle, 220); }
  };
  $('#inspHandle').addEventListener('click', () => { if (inspector.classList.contains('open')) closeInspector(); });   // a plain click / Enter on the handle closes it
  inspector.addEventListener('pointerup', inspPointerEnd);
  inspector.addEventListener('pointercancel', inspPointerEnd);
  /* swipe up to dismiss the search sheet, same as the menu: from the handle or backdrop any time, from anywhere on the sheet except a results
     list that still has room to scroll, and from the results list once it has reached its end. Clicking the handle or the backdrop closes it too. */
  let sdrag = null;
  const sheetBackdrop = $('#sheetBackdrop');
  function sheetTouchStart(e){
    if (!S.sr.open || e.touches.length !== 1) { sdrag = null; return; }
    const t = e.touches[0], list = e.target.closest && e.target.closest('#sheetResults');
    const room = list && list.scrollTop + list.clientHeight < list.scrollHeight - 2;
    sdrag = {y:t.clientY, x:t.clientX, ok:!room, active:false, dy:0, t:Date.now(), h:Math.max(sheet.offsetHeight, 320)};
  }
  function sheetTouchMove(e){
    if (!sdrag || !sdrag.ok) return;
    const t = e.touches[0], dy = t.clientY - sdrag.y, dx = t.clientX - sdrag.x;
    if (!sdrag.active) {
      if (dy > -10) { if (dy > 6) sdrag = null; return; }
      if (Math.abs(dx) > Math.abs(dy)) { sdrag = null; return; }
      sdrag.active = true; sInput.blur();
    }
    sdrag.dy = Math.min(0, dy);
    setSheet(Math.max(0, 1 + sdrag.dy / (sdrag.h * 0.7)), false);
    if (e.cancelable) e.preventDefault();
  }
  function sheetTouchEnd(){
    const m = sdrag;
    sdrag = null;
    if (!m || !m.active) return;
    sheet.style.transition = '';
    if (m.dy < -70 || (Date.now() - m.t < 260 && m.dy < -40)) closeSearch();
    else setSheet(1, true);
  }
  [sheet, sheetBackdrop].forEach(el => {
    el.addEventListener('touchstart', sheetTouchStart, {passive:true});
    el.addEventListener('touchmove', sheetTouchMove, {passive:false});
    el.addEventListener('touchend', sheetTouchEnd);
    el.addEventListener('touchcancel', sheetTouchEnd);
  });
  // mouse / trackpad: scrolling down over the backdrop dismisses as well
  let swd = 0;
  sheetBackdrop.addEventListener('wheel', e => { if (!S.sr.open) return; swd = e.deltaY > 0 ? swd + e.deltaY : 0; if (swd > 60) { swd = 0; closeSearch(); } }, {passive:true});
  /* swipe up to dismiss the popup menu: from the handle or backdrop any time, and from the list once it has reached its end */
  let mdrag = null;
  const resetMenuStyle = () => { menuPop.style.transition = ''; menuPop.style.transform = ''; menuPop.style.opacity = ''; };
  function menuTouchStart(e){
    if (menuPop.hidden || e.touches.length !== 1) { mdrag = null; return; }
    const t = e.touches[0], atEnd = menuPop.scrollTop + menuPop.clientHeight >= menuPop.scrollHeight - 2;
    mdrag = {x:t.clientX, y:t.clientY, ok:!!e.target.closest('.menu-handle') || e.target === menuScrim || atEnd, active:false, dy:0, t:Date.now()};
  }
  function menuTouchMove(e){
    if (!mdrag || !mdrag.ok) return;
    const t = e.touches[0], dy = t.clientY - mdrag.y, dx = t.clientX - mdrag.x;
    if (!mdrag.active) {
      if (dy > -10) { if (dy > 6) mdrag = null; return; }             // moving down: let the list scroll
      if (Math.abs(dx) > Math.abs(dy)) { mdrag = null; return; }
      mdrag.active = true; menuPop.style.transition = 'none';
    }
    mdrag.dy = Math.min(0, dy);
    menuPop.style.transform = 'translateY(' + mdrag.dy + 'px)';
    menuPop.style.opacity = String(Math.max(0.35, 1 + mdrag.dy / 320));
    if (e.cancelable) e.preventDefault();
  }
  function menuTouchEnd(){
    const m = mdrag;
    mdrag = null;
    if (!m || !m.active) return;
    menuPop.style.transition = 'transform 0.2s ease, opacity 0.2s ease';
    if (m.dy < -70 || (Date.now() - m.t < 260 && m.dy < -40)) {
      menuPop.style.transform = 'translateY(-115%)'; menuPop.style.opacity = '0';
      lsSet('openkite-menu-hint', '9'); menuPop.classList.remove('hint');
      setTimeout(() => { closeMenu(); resetMenuStyle(); }, 200);
    } else { menuPop.style.transform = ''; menuPop.style.opacity = ''; setTimeout(() => { menuPop.style.transition = ''; }, 220); }
  }
  [menuPop, menuScrim].forEach(el => {
    el.addEventListener('touchstart', menuTouchStart, {passive:true});
    el.addEventListener('touchmove', menuTouchMove, {passive:false});
    el.addEventListener('touchend', menuTouchEnd);
    el.addEventListener('touchcancel', menuTouchEnd);
  });
  menuPop.addEventListener('click', e => {
    if (e.target.closest('.menu-handle')) { closeMenu(); return; }
    const gb = e.target.closest('.menu-gb');
    if (gb) {
      const st = menuPop.scrollTop, name = gb.dataset.group;
      toggleNavGroup(name); renderNav();
      menuPop.scrollTop = st;
      const again = $('.menu-gb[data-group="' + CSS.escape(name) + '"]', menuPop); if (again) again.focus();
      return;
    }
    const b = e.target.closest('[data-layout]');
    if (!b) return;
    const st = menuPop.scrollTop;
    setNavLayout(b.dataset.layout);
    menuPop.scrollTop = st;
    const again = $('[data-layout="' + b.dataset.layout + '"]', menuPop); if (again) again.focus();
  });
  /* Smooth expand / collapse: the tree is re-rendered, so animate the sections whose state changed. */
  const calmMotion = () => !!(window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches);
  const navSnapshot = (root, secSel, bodySel) => new Map($$(secSel, root).map(sec => { const body = sec.querySelector(bodySel); return [sec.querySelector('.nav-group').dataset.group, {open:!body.hidden, h:body.offsetHeight}]; }));
  function animateNav(prev, root, secSel, bodySel){
    if (!prev.size || calmMotion()) return;
    $$(secSel, root).forEach(sec => {
      const name = sec.querySelector('.nav-group').dataset.group, body = sec.querySelector(bodySel), before = prev.get(name), now = navOpen.has(name);
      if (!before || before.open === now || !body.animate) return;
      const chev = sec.querySelector('.nav-group .icon'), ease = 'cubic-bezier(.2,.8,.2,1)';
      if (chev && chev.animate) chev.animate([{transform:'rotate(' + (now ? 0 : 90) + 'deg)'}, {transform:'rotate(' + (now ? 90 : 0) + 'deg)'}], {duration:220, easing:ease});
      body.style.overflow = 'hidden';
      let anim;
      if (now) {
        const h = body.scrollHeight;
        anim = body.animate([{height:'0px', opacity:0}, {height:h + 'px', opacity:1}], {duration:260, easing:ease});
        Array.from(body.children).slice(0, 10).forEach((k, i) => { if (k.animate) k.animate([{opacity:0, transform:'translateX(-8px)'}, {opacity:1, transform:'none'}], {duration:220, delay:30 + i * 22, easing:ease, fill:'backwards'}); });
      } else {
        body.hidden = false;
        anim = body.animate([{height:before.h + 'px', opacity:1}, {height:'0px', opacity:0}], {duration:220, easing:ease});
      }
      anim.onfinish = anim.oncancel = () => { body.style.overflow = ''; if (!now) body.hidden = true; };
    });
  }
  function renderNav(){
    const active = {view:S.view, kind:S.kind};
    const prevNav = navSnapshot(navTree, '.nav-section', '.nav-items, .nav-grid'), prevMenu = navSnapshot(menuPop, '.menu-sec', '.menu-items, .menu-grid');
    navTree.innerHTML = navTreeHtml(active);
    animateNav(prevNav, navTree, '.nav-section', '.nav-items, .nav-grid');
    menuPop.innerHTML = menuNavHtml(active) +
      '<div class="menu-group">Account</div>' +
      '<button class="menu-item" type="button" data-cluster-open>' + ICON('cluster') + 'Switch cluster…</button>' +
      '<button class="menu-item" type="button" data-theme-toggle aria-pressed="' + (document.documentElement.dataset.theme === 'dark') + '">' + ICON('moon') + 'Dark mode<span class="switch" aria-hidden="true"></span></button>' +
      '<button class="menu-item" type="button" data-open-settings>' + ICON('settings') + 'Settings</button>' +
      '<button type="button" class="menu-handle" aria-label="Close menu. You can also swipe up."><span class="mh-bar" aria-hidden="true"></span></button>';
    animateNav(prevMenu, menuPop, '.menu-sec', '.menu-items, .menu-grid');
    updateCounts();
  }
  navTree.addEventListener('click', e => {
    const tb = e.target.closest('[data-navtab]');
    if (tb) { S.navTab = tb.dataset.navtab; renderNav(); const again = $('[data-navtab="' + CSS.escape(S.navTab) + '"]', navTree); if (again) again.focus(); return; }
    const g = e.target.closest('.nav-group');
    if (!g) return;
    const name = g.dataset.group;
    toggleNavGroup(name);
    renderNav();
    const again = $('.nav-group[data-group="' + CSS.escape(g.dataset.group) + '"]', navTree); if (again) again.focus();
  });

  document.addEventListener('click', e => { const t = e.target.closest('#navTree [data-toast], #menuPop [data-toast]'); if (t) showToast(t.dataset.toast); });
  function applyTheme(t, persist){
    document.documentElement.dataset.theme = t;
    $$('[data-theme-toggle]').forEach(b => { b.setAttribute('aria-pressed', String(t === 'dark')); b.title = t === 'dark' ? 'Switch to light mode' : 'Switch to dark mode'; });
    if (persist) { try { localStorage.setItem('openkite-theme', t); } catch (err) { /* private mode */ } }
  }
  document.addEventListener('click', e => {
    if (!e.target.closest('[data-theme-toggle]')) return;
    const next = document.documentElement.dataset.theme === 'dark' ? 'light' : 'dark';
    applyTheme(next, true);
    showToast(next === 'dark' ? 'Dark mode on' : 'Light mode on');
  });

  function openSidebar(){ sidebar.classList.add('open'); sidebarBackdrop.classList.add('show'); }
  function closeSidebar(){ sidebar.classList.remove('open'); sidebarBackdrop.classList.remove('show'); }
  sidebarBackdrop.addEventListener('click', closeSidebar);

  /* gesture guidance: shown on the first few opens, retired for good once a swipe-up has been used */
  function menuHint(){
    const n = Number(lsGet('openkite-menu-hint', '0')) || 0, show = n < 4;
    menuPop.classList.toggle('hint', show);
    if (show) {
      lsSet('openkite-menu-hint', String(n + 1));
      if (menuPop.animate && !calmMotion()) setTimeout(() => { if (!menuPop.hidden && !mdrag) menuPop.animate([{transform:'translateY(0)'}, {transform:'translateY(-14px)'}, {transform:'translateY(0)'}], {duration:700, easing:'cubic-bezier(.3,.7,.3,1)'}); }, 650);   // a small upward peek shows the motion
    }
  }
  function openMenu(){
    resetMenuStyle();
    menuHint();
    menuPop.hidden = false; menuScrim.hidden = false;
    menuToggle.setAttribute('aria-expanded', 'true');
    const a = $('.menu-item.active', menuPop) || $('.menu-item', menuPop); if (a) a.focus();
  }
  function closeMenu(){ menuPop.hidden = true; menuScrim.hidden = true; menuToggle.setAttribute('aria-expanded', 'false'); }
  menuToggle.addEventListener('click', () => { if (menuPop.hidden) openMenu(); else closeMenu(); });
  menuScrim.addEventListener('click', closeMenu);

  $$('[data-toast]').forEach(el => el.addEventListener('click', () => showToast(el.dataset.toast)));
  $$('[data-refresh]').forEach(btn => btn.addEventListener('click', () => {
    const icon = btn.querySelector('.icon');
    if (icon) icon.style.animation = 'spin 0.7s linear infinite';
    showToast('Resources refreshed');
    if (S.view === 'overview') renderOverview();
    setTimeout(() => { if (icon) icon.style.animation = ''; }, 800);
  }));

  // Left-edge swipe opens the full navigation drawer on mobile; swipe left closes it
  let edge = null;
  document.addEventListener('touchstart', e => {
    const t = e.touches[0], open = sidebar.classList.contains('open');
    edge = (window.innerWidth < 768 && (t.clientX <= 24 || open)) ? {x:t.clientX, y:t.clientY, open} : null;
  }, {passive:true});
  document.addEventListener('touchend', e => {
    if (!edge) return;
    const t = e.changedTouches[0], dx = t.clientX - edge.x, dy = t.clientY - edge.y;
    if (Math.abs(dx) > 56 && Math.abs(dx) > Math.abs(dy) * 1.5) {
      if (!edge.open && dx > 0) openSidebar();
      if (edge.open && dx < 0) closeSidebar();
    }
    edge = null;
  });

  /* ------------------------------------------------------------ log engine */
  function rng(seed){ let s = 7; for (const c of seed) s = (s * 31 + c.charCodeAt(0)) >>> 0; return () => { s = (s * 1664525 + 1013904223) >>> 0; return s / 4294967296; }; }
  const pick = (r, a) => a[Math.floor(r() * a.length)];
  const ROUTES = ['/api/v1/orders', '/api/v1/cart', '/api/v1/checkout', '/healthz', '/metrics', '/api/v1/inventory', '/api/v1/users/me'];
  const FLAVORS = [
    [/coredns/, r => ['INFO', '10.244.4.' + Math.floor(r() * 90) + ':' + (30000 + Math.floor(r() * 9000)) + ' A IN api.internal. udp 41 false 512 NOERROR ' + (r() * 0.4).toFixed(4) + 's']],
    [/prometheus|grafana/, r => [r() < 0.12 ? 'WARN' : 'INFO', pick(r, ['msg="Compaction completed" duration=' + Math.floor(r() * 900) + 'ms', 'msg="WAL checkpoint" segments=' + Math.floor(r() * 30), 'msg="Scrape target up" job=kubelet', 'msg="Query executed" range=5m series=' + Math.floor(r() * 4000)])]],
    [/argocd/, r => [r() < 0.1 ? 'WARN' : 'INFO', pick(r, ['Reconciliation completed app=guestbook comparison=1.2s', 'git fetch origin --tags succeeded', 'Refreshing app status (normal refresh)', 'Updated sync status: Synced -> OutOfSync'])]],
    [/etcd-backup/, r => ['INFO', pick(r, ['snapshot saved size=' + (180 + Math.floor(r() * 40)) + 'MiB', 'uploading to s3://prod-etcd-backups', 'verifying snapshot checksum ok'])]]
  ];
  const isSidecar = c => /envoy|sidecar|reloader/.test(c);
  function genLines(p, container, previous, count){
    const r = rng(p.name + container + (previous ? 'prev' : ''));
    const flavor = FLAVORS.find(f => f[0].test(p.name));
    const lines = [];
    let t = Date.UTC(2025, 5, 12, 10, 40, 0) + (previous ? -3600000 : 0);
    for (let i = 0; i < count; i++) {
      t += 200 + Math.floor(r() * 2600);
      lines.push(makeLine(p, r, t, i, flavor, isSidecar(container), previous));
    }
    return lines;
  }
  function makeLine(p, r, t, i, flavor, sidecar, previous){
    const d = new Date(t), pad = (n, w) => String(n).padStart(w || 2, '0');
    const o = {id:'L' + t + '-' + i, time:pad(d.getUTCHours()) + ':' + pad(d.getUTCMinutes()) + ':' + pad(d.getUTCSeconds()) + '.' + pad(d.getUTCMilliseconds(), 3), level:'INFO', method:null, msg:'', err:false};
    if (p.status === 'CrashLoopBackOff' && (previous || r() < 0.18)) {
      o.level = 'ERROR'; o.err = true;
      o.msg = pick(r, ['panic: connection refused dialing postgres-primary.default:5432', 'failed to acquire lease: context deadline exceeded', 'fatal: unable to load config /etc/worker/config.yaml: permission denied', 'queue consumer exited with status 1']);
    } else if (sidecar) {
      o.msg = pick(r, ['[envoy] upstream cluster outbound|8080 health check passed', '[envoy] config reload ok version=' + Math.floor(r() * 500), '[envoy] request id=' + Math.floor(r() * 1e6).toString(16) + ' 200 via upstream_cx']);
      if (r() < 0.06) { o.level = 'WARN'; o.msg = '[envoy] upstream connect timeout 3000ms'; }
    } else if (flavor) {
      const f = flavor[1](r); o.level = f[0]; o.msg = f[1];
    } else {
      const q = r();
      o.method = r() < 0.7 ? 'GET' : 'POST';
      const route = pick(r, ROUTES);
      if (q < 0.06) { o.level = 'ERROR'; o.err = true; o.msg = route + ' 502 upstream connect error after ' + Math.floor(200 + r() * 900) + 'ms'; }
      else if (q < 0.16) { o.level = 'WARN'; o.method = null; o.msg = 'upstream 10.0.4.22:8080 slow response ' + Math.floor(250 + r() * 400) + 'ms'; }
      else o.msg = route + ' ' + (o.method === 'POST' ? 201 : 200) + ' ' + (r() * 60).toFixed(1) + 'ms';
    }
    return o;
  }

  /* ------------------------------------------------------------------ dock */
  const dock = $('#dock'), dockTabsEl = $('#dockTabs'), dockBody = $('#dockBody'), dropHint = $('#dropHint');
  const dockTabBtn = $('#dockTab'), dockHandle = $('#dockHandle');
  const splitBtn = $('#splitBtn'), swapBtn = $('#swapBtn'), addTabBtn = $('#addTab'), addMenu = $('#addMenu');

  let termSeq = 1;
  const newTerm = (id, label, o) => Object.assign({id, type:'term', label, pinned:true, out:'', hist:[], hi:0, cwd:'/'}, o);
  const tabs = [newTerm('term:cluster', 'Terminal', {cluster:true, out:'<span class="muted">Connected to prod-us-east-1. Type “help” for commands.</span>\n'})];
  const panes = ['term:cluster'];          // tab ids currently visible (1 or 2)
  const els = new Map();                   // tab id -> persistent pane element
  let focus = 0, lastCount = 1;
  const tabById = id => tabs.find(t => t.id === id);
  const activeTab = () => tabById(panes[focus]);
  const isOpen = () => dock.classList.contains('open');
  const narrow = () => window.innerWidth < 768;
  const canHover = () => !!(window.matchMedia && window.matchMedia('(hover:hover)').matches);

  function makeTerminal(){
    const n = ++termSeq;
    const t = newTerm('term:' + n, 'Terminal ' + n, {cluster:true, out:'<span class="muted">Connected to prod-us-east-1. Type “help” for commands.</span>\n'});
    tabs.push(t);
    return t;
  }
  function otherTabFor(id){
    const spare = tabs.find(t => t.id !== id && panes.indexOf(t.id) < 0);
    return spare ? spare.id : makeTerminal().id;
  }

  /* ---- tab strip */
  function renderTabs(){
    const split = panes.length > 1;
    dockTabsEl.innerHTML = tabs.map(t => {
      const pi = panes.indexOf(t.id), act = pi === focus, shown = pi >= 0;
      const badge = split && shown ? '<span class="tab-badge" aria-label="Pane ' + (pi + 1) + '">' + (pi + 1) + '</span>' : '';
      const pin = t.type === 'log' ? '<button class="tab-btn' + (t.pinned ? ' pinned' : '') + '" type="button" data-pin="' + esc(t.id) + '" aria-pressed="' + t.pinned + '" aria-label="' + (t.pinned ? 'Unpin ' : 'Pin ') + esc(t.label) + '" title="' + (t.pinned ? 'Pinned — click to unpin' : 'Pin this tab') + '">' + ICON(t.pinned ? 'pin-fill' : 'pin') + '</button>' : '';
      const close = '<button class="tab-btn" type="button" data-close="' + esc(t.id) + '" aria-label="Close ' + esc(t.label) + '" title="Close tab">' + ICON('close') + '</button>';
      return '<div class="dock-tab' + (act ? ' active' : shown ? ' shown' : '') + (t.type === 'log' && !t.pinned ? ' temp' : '') + '" data-id="' + esc(t.id) + '">' +
        '<button class="tab-main" type="button" role="tab" aria-selected="' + act + '" data-tab="' + esc(t.id) + '" title="' + esc(t.label) + ' — drag to split or reorder">' + ICON(t.type === 'log' ? 'logs' : 'terminal') + '<span class="tab-label">' + esc(t.label) + '</span>' + badge + '</button>' + pin + close + '</div>';
    }).join('');
    const extra = Math.max(0, tabs.length - 1), badge = $('#dockBadge');
    badge.hidden = extra < 1; badge.textContent = extra;
    dockTabsEl.appendChild(addTabBtn);
    splitBtn.setAttribute('aria-pressed', String(split));
    splitBtn.title = split ? 'Close split view' : 'Split view';
    swapBtn.hidden = !split;
    const a = $('.dock-tab.active', dockTabsEl);
    if (a) {
      const l = a.offsetLeft, r = l + a.offsetWidth;
      if (l < dockTabsEl.scrollLeft) dockTabsEl.scrollLeft = Math.max(0, l - 8);
      else if (r > dockTabsEl.scrollLeft + dockTabsEl.clientWidth) dockTabsEl.scrollLeft = r - dockTabsEl.clientWidth + 8;
    }
  }

  /* ---- log pane */
  function updateNote(t){
    if (!t.ui) return;
    const msg = [t.paused ? 'Log stream paused.' : '', t.hlMode ? 'Highlight mode: click or tap lines to mark them.' : ''].filter(Boolean).join(' ');
    t.ui.note.textContent = msg;
    t.ui.note.classList.toggle('show', !!msg);
  }
  function renderLogLine(t, l){
    const q = t.search.trim();
    let msg = esc(l.msg);
    if (q) {
      const re = new RegExp('(' + esc(q).replace(/[.*+?^${}()|[\]\\]/g, '\\$&') + ')', 'ig');
      msg = msg.replace(re, '<mark>$1</mark>');
    }
    return '<div class="log-line' + (t.hl.has(l.id) ? ' hl' : '') + '" data-id="' + l.id + '"><span class="log-time">' + l.time + '</span><span class="log-level ' + l.level.toLowerCase() + '">' + l.level + '</span>' +
      (l.method ? '<span class="log-method">' + l.method + '</span>' : '') + '<span class="log-msg' + (l.err ? ' error' : '') + '">' + msg + '</span></div>';
  }
  const logMatches = (t, l) => { const q = t.search.trim().toLowerCase(); return !q || (l.msg + ' ' + l.level).toLowerCase().includes(q); };
  function renderLogs(t){
    const body = t.ui.body, rows = t.lines.filter(l => logMatches(t, l));
    body.innerHTML = rows.length ? rows.map(l => renderLogLine(t, l)).join('') : '<div class="log-empty">' + (t.search ? 'No log lines match “' + esc(t.search) + '”.' : 'No log output.') + '</div>';
    body.classList.toggle('no-ts', !t.ts);
    body.classList.toggle('hl-mode', t.hlMode);
    body.scrollTop = body.scrollHeight;
  }
  function regen(t){ t.lines = genLines(t.podObj, t.container, t.previous, t.previous ? 60 : 120); t.hl.clear(); }

  function buildLog(t, main){
    const tg = (k, label, on) => '<button class="tool-toggle" type="button" data-k="' + k + '" aria-pressed="' + on + '">' + label + '</button>';
    const ib = (k, icon, label, on) => '<button class="tool-toggle icon-only" type="button" data-k="' + k + '" aria-label="' + label + '" title="' + label + '" aria-pressed="' + on + '">' + ICON(icon) + '</button>';
    main.innerHTML = '<div class="log-tools"><select aria-label="Container">' + t.podObj.ctrs.map(c => '<option' + (c === t.container ? ' selected' : '') + '>' + esc(c) + '</option>').join('') + '</select>' +
      '<label class="search-field log-search">' + ICON('search') + '<input type="search" placeholder="Search logs" aria-label="Search logs" value="' + esc(t.search) + '"></label>' +
      tg('ts', 'Timestamps', t.ts) + tg('previous', 'Previous', t.previous) + tg('follow', 'Follow', t.follow) +
      '<button class="tool-toggle" type="button" data-k="dl">' + ICON('download') + 'Download</button><span class="tools-sep"></span>' +
      ib('hl', 'highlight', 'Highlight lines', t.hlMode) + ib('paused', 'pause', 'Pause stream', t.paused) + ib('clear', 'clear', 'Clear view', false) +
      '</div><div class="dock-note" role="status"></div><div class="log-body"></div>';
    t.ui = {body:$('.log-body', main), note:$('.dock-note', main)};
    const sel = (k, on) => { const b = $('[data-k="' + k + '"]', main); b.setAttribute('aria-pressed', String(on)); };
    $('select', main).addEventListener('change', e => { t.container = e.target.value; regen(t); renderLogs(t); });
    $('.log-search input', main).addEventListener('input', e => { t.search = e.target.value; renderLogs(t); });
    $('.log-tools', main).addEventListener('click', e => {
      const b = e.target.closest('[data-k]');
      if (!b) return;
      const k = b.dataset.k;
      if (k === 'ts' || k === 'follow') { t[k] = !t[k]; sel(k, t[k]); if (k === 'ts') renderLogs(t); else if (t.follow) t.ui.body.scrollTop = t.ui.body.scrollHeight; }
      else if (k === 'previous') { t.previous = !t.previous; sel(k, t.previous); regen(t); renderLogs(t); }
      else if (k === 'hl') { t.hlMode = !t.hlMode; sel(k, t.hlMode); t.ui.body.classList.toggle('hl-mode', t.hlMode); updateNote(t); }
      else if (k === 'paused') { t.paused = !t.paused; sel(k, t.paused); updateNote(t); }
      else if (k === 'clear') { t.lines = []; t.hl.clear(); renderLogs(t); showToast('Log view cleared'); }
      else if (k === 'dl') {
        const text = t.lines.map(l => l.time + ' ' + l.level + ' ' + (l.method ? l.method + ' ' : '') + l.msg).join('\n');
        const a = document.createElement('a');
        a.href = URL.createObjectURL(new Blob([text], {type:'text/plain'}));
        a.download = t.podObj.name + '-' + t.container + '.log';
        document.body.appendChild(a); a.click(); a.remove();
        showToast('Downloaded ' + a.download);
      }
    });
    t.ui.body.addEventListener('click', e => {
      const line = e.target.closest('.log-line');
      if (!line || !t.hlMode) return;
      const id = line.dataset.id;
      if (t.hl.has(id)) t.hl.delete(id); else t.hl.add(id);
      line.classList.toggle('hl');
    });
    updateNote(t);
  }

  /* ---- terminal pane */
  function buildTerm(t, main){
    main.innerHTML = '<div class="term-out"></div><label class="term-prompt"><span class="term-ps"></span><input type="text" autocomplete="off" autocapitalize="off" spellcheck="false" aria-label="Terminal input"></label>';
    const out = $('.term-out', main), input = $('input', main), ps = $('.term-ps', main);
    t.ui = {out, input, ps};
    out.innerHTML = t.out;
    ps.textContent = termPrompt(t);
    input.placeholder = t.cluster ? 'kubectl get pods' : 'ls';
    input.addEventListener('keydown', e => {
      if (e.key === 'ArrowUp' || e.key === 'ArrowDown') {
        e.preventDefault();
        t.hi = Math.max(0, Math.min(t.hist.length, t.hi + (e.key === 'ArrowUp' ? -1 : 1)));
        input.value = t.hist[t.hi] || '';
        return;
      }
      if (e.key !== 'Enter') return;
      const cmd = input.value.trim();
      input.value = '';
      if (!cmd) return;
      t.hist.push(cmd); t.hi = t.hist.length;
      if (cmd === 'clear') { t.out = ''; out.innerHTML = ''; return; }
      const prompt = termPrompt(t);
      let res;
      if (t.cluster) {
        const a = cmd.split(/\s+/);
        if (cmd === 'help') res = 'kubectl get pods|deployments|nodes|namespaces [-n ns]\nkubectl describe pod <name>\nkubectl logs <pod>             opens a log tab\nkubectl exec -it <pod> -- sh   opens a shell tab\nkubectl delete pod <name>\nkubectl config current-context\nclear';
        else if (a[0] === 'kubectl') res = kubectl(a.slice(1));
        else res = errLine('command not found: ' + a[0] + ' (try “help”)');
      } else {
        res = podShell(cmd, t);
        if (res === null) return;
      }
      t.out += '<span class="cmd">' + esc(prompt) + ' ' + esc(cmd) + '</span>\n' + (res ? asHtml(res) + '\n' : '');
      out.innerHTML = t.out;
      ps.textContent = termPrompt(t);
      out.scrollTop = out.scrollHeight;
    });
  }

  /* ---- pane elements + layout */
  function buildPane(t){
    const el = document.createElement('div');
    el.className = 'dpane ' + t.type;
    el.dataset.tab = t.id;
    el.innerHTML = '<div class="dpane-head"><span class="dpane-title">' + ICON(t.type === 'log' ? 'logs' : 'terminal') + '<span>' + esc(t.label) + '</span></span><button class="dpane-x" type="button" aria-label="Close pane" title="Close pane">' + ICON('close') + '</button></div><div class="dpane-main"></div>';
    const main = $('.dpane-main', el);
    if (t.type === 'log') buildLog(t, main); else buildTerm(t, main);
    return el;
  }
  function retire(c, animate){
    if (!animate || !c.animate) { c.remove(); return; }
    c.classList.add('leaving');
    const a = c.animate([{flexGrow:1, opacity:1}, {flexGrow:0.001, opacity:0}], {duration:220, easing:'ease'});
    a.onfinish = () => { if (c.classList.contains('leaving')) c.remove(); };
  }
  function layout(){
    const animate = panes.length !== lastCount;
    lastCount = panes.length;
    const want = panes.map(id => { let e = els.get(id); if (!e) { e = buildPane(tabById(id)); els.set(id, e); } return e; });
    const visible = () => Array.from(dockBody.children).filter(c => !c.classList.contains('leaving'));
    const had = visible();
    had.forEach(c => { if (!want.includes(c)) retire(c, animate); });
    want.forEach((e, i) => {
      if (e.classList.contains('leaving')) { e.classList.remove('leaving'); if (e.getAnimations) e.getAnimations().forEach(a => a.cancel()); }
      const cur = visible();
      if (cur[i] === e) return;
      dockBody.insertBefore(e, cur[i] || null);
      if (!had.includes(e)) {
        const t = tabById(e.dataset.tab);
        if (t.type === 'log') renderLogs(t); else t.ui.out.scrollTop = t.ui.out.scrollHeight;
        if (animate && e.animate) e.animate([{flexGrow:0.001, opacity:0}, {flexGrow:1, opacity:1}], {duration:300, easing:'cubic-bezier(.2,.8,.2,1)'});
      }
    });
    dockBody.classList.toggle('split', panes.length > 1);
    panes.forEach((id, i) => els.get(id).classList.toggle('focused', panes.length > 1 && i === focus));
  }
  function flip(fn){
    const before = new Map(Array.from(dockBody.children).map(c => [c, c.getBoundingClientRect()]));
    fn();
    before.forEach((r, c) => {
      if (!c.isConnected || !c.animate) return;
      const n = c.getBoundingClientRect(), dx = r.left - n.left, dy = r.top - n.top;
      if (dx || dy) c.animate([{transform:'translate(' + dx + 'px,' + dy + 'px)'}, {transform:'none'}], {duration:300, easing:'cubic-bezier(.2,.8,.2,1)'});
    });
  }

  /* ---- tab / pane operations */
  function setFocus(i){
    if (i === focus) return;
    focus = i;
    panes.forEach((id, k) => els.get(id).classList.toggle('focused', panes.length > 1 && k === focus));
    renderTabs();
  }
  function showTab(id){
    const i = panes.indexOf(id);
    if (i >= 0) focus = i; else panes[focus] = id;
    layout(); renderTabs();
    const t = tabById(id);
    if (isOpen() && t.type === 'term' && canHover()) t.ui.input.focus();
  }
  function toggleSplit(){
    if (panes.length > 1) { const keep = panes[focus]; panes.splice(0, panes.length, keep); focus = 0; }
    else { panes.push(otherTabFor(panes[0])); focus = 1; }
    layout(); renderTabs();
  }
  function swapPanes(newFocus){
    if (panes.length < 2) return;
    flip(() => { panes.reverse(); focus = newFocus == null ? 1 - focus : newFocus; layout(); renderTabs(); });
  }
  function closeTab(id){
    const t = tabById(id);
    if (!t) return;
    const i = tabs.indexOf(t), pi = panes.indexOf(id);
    tabs.splice(i, 1);
    els.delete(id);
    if (!tabs.length) { panes.length = 0; focus = 0; layout(); renderTabs(); setDock(false); return; }
    if (pi >= 0) {
      if (panes.length > 1) { panes.splice(pi, 1); focus = 0; }
      else { panes[0] = tabs[Math.min(i, tabs.length - 1)].id; focus = 0; }
    }
    layout(); renderTabs();
  }
  function dropTabsFor(resId){
    tabs.filter(t => t.podObj && t.podObj.id === resId).forEach(t => closeTab(t.id));
  }
  function ensureCluster(){
    let t = tabById('term:cluster');
    if (!t) { t = newTerm('term:cluster', 'Terminal', {cluster:true, out:'<span class="muted">Connected to ' + S.cluster + '. Type “help” for commands.</span>\n'}); tabs.unshift(t); }
    return t;
  }
  function setDock(open){
    if (open && !tabs.length) { const t = ensureCluster(); panes.splice(0, panes.length, t.id); focus = 0; layout(); renderTabs(); }
    dock.classList.toggle('open', open);
    dock.setAttribute('aria-hidden', String(!open));
    document.body.classList.toggle('dock-open', open);
    [dockTabBtn, dockHandle].forEach(b => b.setAttribute('aria-pressed', String(open)));
    dockTabBtn.classList.toggle('active', open);
    dockHandle.setAttribute('aria-label', open ? 'Hide logs and terminal' : 'Show logs and terminal');
    if (open) {
      dock.classList.add('opening'); setTimeout(() => dock.classList.remove('opening'), 240);
      panes.forEach(id => { const t = tabById(id); if (t.type === 'log') t.ui.body.scrollTop = t.ui.body.scrollHeight; });
    }
  }
  function openLog(p, container, label){
    const id = 'log:' + p.id;
    const existing = tabById(id);
    if (existing && container && existing.container !== container) {
      existing.container = container; regen(existing);
      const pe = els.get(id); if (pe) { $('select', pe).value = container; renderLogs(existing); }
    }
    if (!existing) {
      const tab = {id, type:'log', label:label || p.name, pinned:false, podObj:p, container:container || p.ctrs[0], previous:false, ts:true, follow:true, search:'', paused:false, hlMode:false, lines:[], hl:new Set()};
      regen(tab);
      const tmp = tabs.findIndex(t => t.type === 'log' && !t.pinned && panes.indexOf(t.id) < 0 || t.type === 'log' && !t.pinned && t.id === panes[focus]);
      if (tmp >= 0) { const old = tabs[tmp]; els.delete(old.id); tabs.splice(tmp, 1, tab); const pi = panes.indexOf(old.id); if (pi >= 0) panes[pi] = id; }
      else tabs.push(tab);
    }
    setDock(true); showTab(id);
  }
  function openShell(p){
    const id = 'sh:' + p.id;
    if (!tabById(id)) tabs.push(newTerm(id, 'sh · ' + p.name, {podObj:p, cwd:'/app', out:'<span class="muted">Defaulted container “' + esc(p.ctrs[0]) + '”. Type “help” for commands.</span>\n'}));
    setDock(true); showTab(id);
  }
  function openClusterShell(){ ensureCluster(); setDock(true); showTab('term:cluster'); }

  /* ---- new-tab menu */
  S.am = {mode:'logs', q:'', idx:0, items:[]};
  function closeAddMenu(){ addMenu.hidden = true; addTabBtn.setAttribute('aria-expanded', 'false'); }
  const AM_KINDS = ['pods', 'deployments', 'statefulsets', 'daemonsets', 'jobs', 'cronjobs', 'replicasets'];
  function amTarget(kind, r){
    if (kind === 'pods') return r;
    if (kind === 'replicasets') return DATA.pods.find(p => p.ns === r.ns && p.name.indexOf(r.name + '-') === 0);
    if (kind === 'cronjobs') { const j = DATA.jobs.find(x => x.ns === r.ns && x.name.indexOf(r.name + '-') === 0); return j ? podsOf('jobs', j)[0] : null; }
    return podsOf(kind, r)[0];
  }
  function amRender(){
    const q = S.am.q.trim().toLowerCase(), list = $('#amList');
    const rows = [];
    AM_KINDS.forEach(k => {
      let items = DATA[k].map(r => ({k, r, p:amTarget(k, r)})).filter(x => x.p && (!q || (x.r.name + ' ' + (x.r.ns || '') + ' ' + KINDS[k].one).toLowerCase().indexOf(q) >= 0));
      if (!q) items = items.slice(0, 4);
      if (items.length) rows.push({head:KINDS[k].label, items});
    });
    S.am.items = [];
    list.innerHTML = rows.map(g => '<div class="menu-group">' + esc(g.head) + '</div>' + g.items.map(x => {
      const i = S.am.items.push(x) - 1, st = statusOf(x.k, x.r);
      return '<button type="button" class="am-row" role="option" id="am' + i + '" data-i="' + i + '" aria-selected="false">' + ICON(KINDS[x.k].icon) + '<span class="am-main"><span class="am-l">' + (q ? hl(x.r.name, q) : esc(x.r.name)) + '</span><span class="am-s">' + esc((x.r.ns ? x.r.ns + ' · ' : '') + (x.k === 'pods' ? x.r.node : x.p.name)) + '</span></span>' + (st ? pill(st[0], st[1]) : '') + '</button>';
    }).join('')).join('') || '<p class="muted-note am-empty">No pod or workload matches “' + esc(S.am.q.trim()) + '”.</p>';
    S.am.idx = 0; amMark();
  }
  function amMark(){
    $$('.am-row', addMenu).forEach((b, i) => b.setAttribute('aria-selected', String(i === S.am.idx)));
    const cur = $('#am' + S.am.idx), list = $('#amList');
    if (cur) { const t = cur.offsetTop - 34, bt = cur.offsetTop + cur.offsetHeight + 6; if (t < list.scrollTop) list.scrollTop = Math.max(0, t); else if (bt > list.scrollTop + list.clientHeight) list.scrollTop = bt - list.clientHeight; }
  }
  function amPick(i){
    const x = S.am.items[i];
    if (!x) return;
    closeAddMenu();
    if (S.am.mode === 'shell') return openShell(x.p);
    openLog(x.p, undefined, x.k === 'pods' ? undefined : x.r.name);
  }
  function openAddMenu(){
    closePops();
    S.am.q = '';
    addMenu.innerHTML = '<div class="am-search"><label class="search-field">' + ICON('search') + '<input type="search" id="amInput" placeholder="Search pods, deployments, jobs…" aria-label="Search workloads for logs" autocomplete="off" spellcheck="false" role="combobox" aria-expanded="true" aria-controls="amList"></label></div>' +
      '<div class="am-modes" role="group" aria-label="Open as"><button type="button" class="chip' + (S.am.mode === 'logs' ? ' active' : '') + '" data-mode="logs" aria-pressed="' + (S.am.mode === 'logs') + '">Logs</button><button type="button" class="chip' + (S.am.mode === 'shell' ? ' active' : '') + '" data-mode="shell" aria-pressed="' + (S.am.mode === 'shell') + '">Shell</button></div>' +
      '<div class="am-list" id="amList" role="listbox"></div>' +
      '<div class="am-foot"><button type="button" class="cm-act" data-new="term">' + ICON('terminal') + 'New terminal</button></div>';
    amRender();
    placePop(addMenu, addTabBtn, false);
    addTabBtn.setAttribute('aria-expanded', 'true');
    const inp = $('#amInput');
    if (inp && window.matchMedia && window.matchMedia('(hover:hover)').matches) inp.focus();
  }
  addTabBtn.addEventListener('click', () => { if (addMenu.hidden) openAddMenu(); else closeAddMenu(); });
  addMenu.addEventListener('input', e => { if (e.target.id === 'amInput') { S.am.q = e.target.value; amRender(); } });
  addMenu.addEventListener('keydown', e => {
    if (e.target.id !== 'amInput') return;
    const n = S.am.items.length;
    if (e.key === 'ArrowDown') { e.preventDefault(); if (n) { S.am.idx = (S.am.idx + 1) % n; amMark(); } }
    else if (e.key === 'ArrowUp') { e.preventDefault(); if (n) { S.am.idx = (S.am.idx - 1 + n) % n; amMark(); } }
    else if (e.key === 'Enter') { e.preventDefault(); amPick(S.am.idx); }
    else if (e.key === 'Tab' && !e.shiftKey) { e.preventDefault(); S.am.mode = S.am.mode === 'logs' ? 'shell' : 'logs'; $$('[data-mode]', addMenu).forEach(b => { const on = b.dataset.mode === S.am.mode; b.classList.toggle('active', on); b.setAttribute('aria-pressed', String(on)); }); }
  });
  addMenu.addEventListener('click', e => {
    const m = e.target.closest('[data-mode]');
    if (m) { S.am.mode = m.dataset.mode; $$('[data-mode]', addMenu).forEach(b => { const on = b === m; b.classList.toggle('active', on); b.setAttribute('aria-pressed', String(on)); }); const i = $('#amInput'); if (i) i.focus(); return; }
    const row = e.target.closest('.am-row');
    if (row) return amPick(Number(row.dataset.i));
    if (e.target.closest('[data-new]')) { closeAddMenu(); const t = makeTerminal(); setDock(true); showTab(t.id); }
  });
  addMenu.addEventListener('pointermove', e => { const row = e.target.closest('.am-row'); if (row && Number(row.dataset.i) !== S.am.idx) { S.am.idx = Number(row.dataset.i); $$('.am-row', addMenu).forEach((b, i) => b.setAttribute('aria-selected', String(i === S.am.idx))); } });
  document.addEventListener('click', e => { if (!addMenu.hidden && !e.target.closest('#addMenu') && !e.target.closest('#addTab')) closeAddMenu(); });

  /* ---- wiring */
  let suppressClick = false;
  dockTabsEl.addEventListener('click', e => {
    if (suppressClick) { suppressClick = false; return; }
    const b = e.target.closest('button');
    if (!b) return;
    if (b.dataset.tab) showTab(b.dataset.tab);
    else if (b.dataset.close) closeTab(b.dataset.close);
    else if (b.dataset.pin) { const t = tabById(b.dataset.pin); t.pinned = !t.pinned; renderTabs(); showToast(t.pinned ? 'Tab pinned' : 'Tab is temporary again'); }
  });
  dockBody.addEventListener('pointerdown', e => {
    const p = e.target.closest('.dpane');
    if (p) { const i = panes.indexOf(p.dataset.tab); if (i >= 0) setFocus(i); }
  }, true);
  dockBody.addEventListener('click', e => {
    const x = e.target.closest('.dpane-x');
    if (!x) return;
    const i = panes.indexOf(x.closest('.dpane').dataset.tab);
    if (panes.length > 1 && i >= 0) { panes.splice(i, 1); focus = 0; layout(); renderTabs(); }
  });
  splitBtn.addEventListener('click', toggleSplit);
  swapBtn.addEventListener('click', () => swapPanes());
  dockTabBtn.addEventListener('click', () => setDock(!isOpen()));
  dockHandle.addEventListener('click', () => setDock(!isOpen()));
  $('#dockClose').addEventListener('click', () => setDock(false));
  $$('[data-open-tab="terminal"]').forEach(b => b.addEventListener('click', openClusterShell));
  document.addEventListener('keydown', e => { if (e.ctrlKey && (e.key === '`' || e.code === 'Backquote')) { e.preventDefault(); setDock(!isOpen()); } });

  /* ---- drag a tab: reorder in the strip, or drop on the stage to split / swap */
  let drag = null;
  function dropTarget(x, y){
    const sr = dockTabsEl.getBoundingClientRect();
    if (y >= sr.top - 6 && y <= sr.bottom + 6 && x >= sr.left && x <= sr.right) {
      const others = $$('.dock-tab', dockTabsEl).filter(el => el.dataset.id !== drag.id);
      if (!others.length) return null;
      const ref = others.find(el => { const r = el.getBoundingClientRect(); return x < r.left + r.width / 2; });
      return {mode:'reorder', ref:ref || others[others.length - 1], after:!ref};
    }
    const br = dockBody.getBoundingClientRect();
    if (x >= br.left && x <= br.right && y >= br.top && y <= br.bottom) {
      const frac = narrow() ? (y - br.top) / br.height : (x - br.left) / br.width;
      return {mode:panes.length > 1 ? 'pane' : 'split', zone:frac < 0.5 ? 0 : 1};
    }
    return null;
  }
  function showHint(tg){
    $$('.dock-tab', dockTabsEl).forEach(el => el.classList.remove('drop-before', 'drop-after'));
    if (!tg || tg.mode === 'reorder') {
      dropHint.classList.remove('show');
      if (tg) tg.ref.classList.add(tg.after ? 'drop-after' : 'drop-before');
      return;
    }
    const vert = narrow(), z = tg.zone;
    dropHint.style.left = vert ? '0' : (z * 50) + '%';
    dropHint.style.width = vert ? '100%' : '50%';
    dropHint.style.top = vert ? (z * 50) + '%' : '0';
    dropHint.style.height = vert ? '50%' : '100%';
    const j = panes.indexOf(drag.id);
    dropHint.textContent = tg.mode === 'split' ? (vert ? (z ? 'Split below' : 'Split above') : (z ? 'Split right' : 'Split left')) : j === z ? 'Already here' : j >= 0 ? 'Swap panes' : 'Open here';
    dropHint.classList.add('show');
  }
  function startDrag(e){
    drag.started = true;
    const t = tabById(drag.id);
    const g = document.createElement('div');
    g.className = 'tab-ghost';
    g.innerHTML = ICON(t.type === 'log' ? 'logs' : 'terminal') + '<span>' + esc(t.label) + '</span>';
    document.body.appendChild(g);
    drag.ghost = g;
    dock.classList.add('dragging');
    document.body.classList.add('tab-dragging');
    const src = $('.dock-tab[data-id="' + CSS.escape(drag.id) + '"]', dockTabsEl); if (src) src.classList.add('dragging');
    moveDrag(e);
  }
  function moveDrag(e){
    drag.ghost.style.left = e.clientX + 'px';
    drag.ghost.style.top = e.clientY + 'px';
    drag.target = dropTarget(e.clientX, e.clientY);
    showHint(drag.target);
  }
  function applyDrop(tg){
    const id = drag.id;
    if (tg.mode === 'reorder') {
      const from = tabs.findIndex(t => t.id === id), moved = tabs.splice(from, 1)[0];
      let to = tabs.findIndex(x => x.id === tg.ref.dataset.id);
      if (tg.after) to++;
      tabs.splice(to, 0, moved);
      renderTabs();
    } else if (tg.mode === 'split') {
      const other = panes[0] !== id ? panes[0] : otherTabFor(id);
      panes.splice(0, panes.length, ...(tg.zone === 0 ? [id, other] : [other, id]));
      focus = tg.zone;
      layout(); renderTabs();
    } else {
      const j = panes.indexOf(id), i = tg.zone;
      if (j === i) { setFocus(i); return; }
      if (j >= 0) swapPanes(i);
      else { panes[i] = id; focus = i; layout(); renderTabs(); }
    }
  }
  function endDrag(apply){
    if (!drag) return;
    clearTimeout(drag.timer);
    const d = drag;
    drag = null;
    if (!d.started) return;
    d.ghost.remove();
    dock.classList.remove('dragging');
    document.body.classList.remove('tab-dragging');
    dropHint.classList.remove('show');
    $$('.dock-tab', dockTabsEl).forEach(el => el.classList.remove('drop-before', 'drop-after', 'dragging'));
    suppressClick = true; setTimeout(() => { suppressClick = false; }, 0);
    if (apply && d.target) { drag = d; applyDrop(d.target); drag = null; }
  }
  dockTabsEl.addEventListener('pointerdown', e => {
    const main = e.target.closest('.tab-main');
    if (!main || e.button > 0) return;
    drag = {id:main.dataset.tab, sx:e.clientX, sy:e.clientY, pid:e.pointerId, type:e.pointerType, started:false, target:null};
    if (e.pointerType === 'touch') drag.timer = setTimeout(() => { if (drag && !drag.started) startDrag(e); }, 300);
  });
  document.addEventListener('pointermove', e => {
    if (!drag || e.pointerId !== drag.pid) return;
    if (!drag.started) {
      const moved = Math.hypot(e.clientX - drag.sx, e.clientY - drag.sy);
      if (drag.type === 'touch') { if (moved > 10) { clearTimeout(drag.timer); drag = null; } return; }
      if (moved < 6) return;
      startDrag(e);
    }
    moveDrag(e);
  });
  document.addEventListener('pointerup', e => { if (drag && e.pointerId === drag.pid) endDrag(true); });
  document.addEventListener('pointercancel', e => { if (drag && e.pointerId === drag.pid) endDrag(false); });
  document.addEventListener('touchmove', e => { if (drag && drag.started) e.preventDefault(); }, {passive:false});
  document.addEventListener('keydown', e => { if (e.key === 'Escape' && drag && drag.started) endDrag(false); });

  // follow: stream a new line every couple of seconds into each visible log pane
  setInterval(() => {
    if (!isOpen()) return;
    panes.forEach(id => {
      const t = tabById(id);
      if (t.type !== 'log' || !t.follow || t.paused || t.previous) return;
      const now = Date.now(), r = rng(t.podObj.name + now);
      const l = makeLine(t.podObj, r, now, t.lines.length, FLAVORS.find(f => f[0].test(t.podObj.name)), isSidecar(t.container), false);
      const d = new Date(), p2 = n => String(n).padStart(2, '0');
      l.time = p2(d.getHours()) + ':' + p2(d.getMinutes()) + ':' + p2(d.getSeconds()) + '.' + String(d.getMilliseconds()).padStart(3, '0');
      t.lines.push(l); if (t.lines.length > 400) t.lines.shift();
      if (logMatches(t, l)) {
        const empty = $('.log-empty', t.ui.body); if (empty) empty.remove();
        t.ui.body.insertAdjacentHTML('beforeend', renderLogLine(t, l));
        t.ui.body.scrollTop = t.ui.body.scrollHeight;
      }
    });
  }, 2200);


  /* -------------------------------------------------------------- terminals */
  function termPrompt(t){ return t.cluster ? '$' : 'root@' + t.podObj.name.split('-').slice(0, 2).join('-') + ':' + t.cwd + '#'; }
  function table(headRow, rows){
    const all = [headRow].concat(rows), w = headRow.map((_, i) => Math.max(...all.map(r => String(r[i]).length)) + 2);
    return all.map(r => r.map((c, i) => String(c).padEnd(w[i])).join('').trimEnd()).join('\n');
  }
  const errLine = m => '<span class="err">' + esc(m) + '</span>';
  function kubectl(args){
    const verb = args[0], res = args[1] || '';
    const nsI = args.findIndex(a => a === '-n' || a === '--namespace'), ns = nsI >= 0 ? args[nsI + 1] : null;
    const rest = args.slice(2).filter(a => !a.startsWith('-') && a !== ns);
    const isPod = /^(po|pods?)$/.test(res);
    if (verb === 'get' && isPod) return table(['NAME', 'READY', 'STATUS', 'RESTARTS', 'AGE'], DATA.pods.filter(p => !ns || p.ns === ns).map(p => [p.name, p.ctrs.length + '/' + p.ctrs.length, p.status, p.restarts, fmtAge(p.age)]));
    if (verb === 'get' && /^(deploy|deployments?)$/.test(res)) return table(['NAME', 'READY', 'AGE'], DATA.deployments.filter(d => !ns || d.ns === ns).map(d => [d.name, d.have + '/' + d.want, fmtAge(d.age)]));
    if (verb === 'get' && /^(ns|namespaces?)$/.test(res)) return table(['NAME', 'STATUS'], Array.from(new Set(DATA.pods.map(p => p.ns))).map(n => [n, 'Active']));
    if (verb === 'get' && /^nodes?$/.test(res)) return table(['NAME', 'STATUS', 'VERSION'], Array.from(new Set(DATA.pods.map(p => p.node))).map(n => [n, 'Ready', 'v1.29.4']));
    if (verb === 'config' && res === 'current-context') return 'prod-us-east-1';
    if (verb === 'logs' && res) {
      const p = DATA.pods.find(x => x.name === res);
      if (!p) return errLine('Error from server (NotFound): pods "' + res + '" not found');
      openLog(p); return 'Opened logs for ' + p.name + ' in a new tab.';
    }
    if (verb === 'exec') {
      const p = DATA.pods.find(x => args.indexOf(x.name) > 0);
      if (!p) return errLine('Error from server (NotFound): pod not found');
      openShell(p); return 'Attached shell to ' + p.name + ' in a new tab.';
    }
    if (verb === 'delete' && isPod && rest[0]) {
      const p = DATA.pods.find(x => x.name === rest[0]);
      if (!p) return errLine('Error from server (NotFound): pods "' + rest[0] + '" not found');
      removeResource(p.id); return 'pod "' + p.name + '" deleted';
    }
    if (verb === 'describe' && isPod && rest[0]) {
      const p = DATA.pods.find(x => x.name === rest[0]);
      return p ? ['Name:          ' + p.name, 'Namespace:     ' + p.ns, 'Node:          ' + p.node, 'IP:            ' + p.ip, 'Controlled By: ' + p.ctrl, 'QoS Class:     ' + p.qos, 'Status:        ' + p.status].join('\n') : errLine('Error from server (NotFound)');
    }
    return errLine('error: unsupported in this prototype: kubectl ' + args.join(' '));
  }
  function podShell(cmd, t){
    const parts = cmd.split(/\s+/), c = parts[0], a = parts.slice(1), p = t.podObj;
    switch (c) {
      case 'help': return 'ls  pwd  cd  whoami  hostname  date  uname  ps  env  cat /etc/os-release  curl  echo  clear  exit';
      case 'pwd': return t.cwd;
      case 'cd': t.cwd = a[0] || '/'; return '';
      case 'ls': return t.cwd === '/app' ? 'config.yaml  entrypoint.sh  server  static' : 'app  bin  dev  etc  home  proc  tmp  usr  var';
      case 'whoami': return 'root';
      case 'hostname': return p.name;
      case 'date': return new Date().toUTCString();
      case 'uname': return 'Linux ' + p.name + ' 6.1.0-k8s #1 SMP x86_64 GNU/Linux';
      case 'ps': return 'PID  USER  TIME  COMMAND\n  1  root  0:42  /app/server --config /app/config.yaml\n 37  root  0:00  ps';
      case 'env': return 'HOSTNAME=' + p.name + '\nKUBERNETES_SERVICE_HOST=10.96.0.1\nPOD_NAMESPACE=' + p.ns + '\nPOD_IP=' + p.ip;
      case 'cat': return a[0] === '/etc/os-release' ? 'NAME="Alpine Linux"\nVERSION_ID=3.19.1' : errLine('cat: ' + (a[0] || '') + ': No such file or directory');
      case 'curl': return '{"status":"ok","pod":"' + p.name + '"}';
      case 'echo': return a.join(' ');
      case 'exit': closeTab(t.id); return null;
      default: return errLine('sh: ' + c + ': not found');
    }
  }
  const asHtml = s => s.startsWith('<span class="err">') ? s : esc(s);
  /* ------------------------------------------------------ pull down to search */
  // Dragging down from the top of any page reveals the search sheet (replaces pull-to-refresh).
  let tp = null;
  main.addEventListener('touchstart', e => {
    if (S.sr.open || !menuPop.hidden || sidebar.classList.contains('open') || e.touches.length !== 1 || main.scrollTop > 0 || ovDrag || (typeof drag !== 'undefined' && drag) || e.target.closest('.menu-pop,.menu-scrim,.nav-controls,.row-menu,.chart-svg,.dock,.inspector,.ov-grip,.ov-w.dragging,.ov-w-bar,.app-card,.range-tabs,.tab-ghost,input,select,textarea')) { tp = null; return; }
    tp = {x:e.touches[0].clientX, y:e.touches[0].clientY, active:false, p:0, h:Math.max(sheet.offsetHeight, 320)};
  }, {passive:true});
  main.addEventListener('touchmove', e => {
    if (!tp) return;
    if (!menuPop.hidden || sidebar.classList.contains('open')) { if (tp.active) { sheet.classList.remove('pulling'); closeSearch(); } tp = null; return; }
    if (ovDrag || (typeof drag !== 'undefined' && drag)) { if (tp.active) { tp = null; sheet.classList.remove('pulling'); closeSearch(); } else tp = null; return; }
    const dy = e.touches[0].clientY - tp.y, dx = e.touches[0].clientX - tp.x;
    if (!tp.active) {
      if (dy < 14 || main.scrollTop > 0) { if (Math.abs(dx) > 14 || dy < -6) tp = null; return; }
      if (Math.abs(dy) < Math.abs(dx) * 1.5) { tp = null; return; }
      tp.active = true;
      sheet.classList.add('pulling'); document.body.classList.add('searching'); renderScopes(); runSearch();
    }
    tp.p = Math.max(0, Math.min(1, (dy - 14) / (tp.h * 0.55)));
    setSheet(tp.p, false);
    if (e.cancelable) e.preventDefault();
  }, {passive:false});
  function touchPullEnd(){
    const t = tp;
    tp = null;
    if (!t || !t.active) return;
    sheet.style.transition = '';
    sheet.classList.remove('pulling');
    if (t.p > 0.35) openSearch(''); else closeSearch();
  }
  main.addEventListener('touchend', touchPullEnd);
  main.addEventListener('touchcancel', touchPullEnd);
  /* mouse / trackpad: keep scrolling up once you are already at the top of the page to pull the search sheet down */
  let wp = null, wpTimer = 0, wpLast = 0;
  const canScrollUp = el => { for (let n = el; n && n !== main; n = n.parentElement) { if (n.scrollTop > 0 && n.scrollHeight > n.clientHeight && /(auto|scroll)/.test(getComputedStyle(n).overflowY)) return true; } return false; };
  main.addEventListener('wheel', e => {
    const nowT = Date.now(), fresh = nowT - wpLast > 320;
    wpLast = nowT;
    if (S.sr.open || e.ctrlKey || Math.abs(e.deltaY) < Math.abs(e.deltaX)) return;
    if (!menuPop.hidden || sidebar.classList.contains('open') || ovDrag || (typeof drag !== 'undefined' && drag)) return;
    if (e.target.closest && e.target.closest('input,textarea,select,.menu-pop,.row-menu')) return;
    const atTop = main.scrollTop <= 1;   // fractional scroll positions on hi-dpi screens
    if (!wp) {
      // A pull only counts when a scroll gesture *starts* at the top. Scrolling up from far down and arriving at the top never opens search by
      // accident: pause for a beat (the gesture ends), then scroll up again.
      if (!fresh || e.deltaY >= 0 || !atTop || canScrollUp(e.target)) return;
      wp = {acc:0, active:false, grace:0};
    }
    clearTimeout(wpTimer);
    const settle = () => { const w = wp; wp = null; if (w && w.active && !S.sr.open) { sheet.classList.remove('pulling'); sheet.style.transition = ''; closeSearch(); } };
    wpTimer = setTimeout(settle, 260);
    if (!atTop) return settle();
    if (e.deltaY > 0) { wp.acc = Math.max(0, wp.acc - e.deltaY); if (!wp.acc) return settle(); }
    else wp.acc += -e.deltaY;
    const eff = wp.acc - wp.grace;
    if (!wp.active && eff > 24) { wp.active = true; sheet.classList.add('pulling'); document.body.classList.add('searching'); renderScopes(); runSearch(); }
    if (wp.active) {
      setSheet(Math.max(0, Math.min(1, eff / 260)), false);
      e.preventDefault();
      if (eff >= 160) { clearTimeout(wpTimer); wp = null; sheet.classList.remove('pulling'); sheet.style.transition = ''; openSearch(''); }
    }
  }, {passive:false});

  /* ------------------------------------------------------------ Argo CD */
  const SHA = s => { let h = hashOf(s); let o = ''; for (let i = 0; i < 7; i++) { o += '0123456789abcdef'[h & 15]; h = (h >>> 4) ^ (h * 2654435761 >>> 0); } return o; };
  const R_ = (kind, name, ns, sync, health, msg) => ({kind, name, ns, sync: sync || 'Synced', health: health || 'Healthy', msg: msg || ''});
  const std = (n, ns, extra) => [R_('Deployment', n, ns), R_('Service', n, ns), R_('ConfigMap', n + '-config', ns)].concat(extra || []);
  const SRC = {helm:{label:'Helm', icon:'helm'}, kustomize:{label:'Kustomize', icon:'kustomize'}, dir:{label:'Directory', icon:'repo'}};
  const ARGO = [
    {name:'guestbook', project:'demo-apps', ns:'default', set:'guestbook-envs', last:2, auto:'auto-sync · prune', sync:'Synced', health:'Healthy',
      src:{type:'helm', repo:'github.com/acme/guestbook', rev:'1.4.2', chart:'guestbook', release:'guestbook', values:['values.yaml', 'values-prod.yaml'], params:{'replicaCount':'3', 'image.tag':'v1.4.2'}},
      res:[R_('Deployment', 'guestbook-ui', 'default'), R_('Service', 'guestbook-ui', 'default'), R_('ConfigMap', 'guestbook-config', 'default'), R_('Ingress', 'guestbook', 'default')]},
    {name:'guestbook-staging', project:'demo-apps', ns:'staging', set:'guestbook-envs', last:5, auto:'auto-sync', sync:'Synced', health:'Healthy',
      src:{type:'helm', repo:'github.com/acme/guestbook', rev:'1.5.0-rc1', chart:'guestbook', release:'guestbook-staging', values:['values.yaml', 'values-staging.yaml'], params:{'replicaCount':'2', 'image.tag':'v1.5.0-rc1'}}, res:std('guestbook-ui', 'staging')},
    {name:'guestbook-dev', project:'demo-apps', ns:'dev', set:'guestbook-envs', last:17, auto:'manual', sync:'OutOfSync', health:'Healthy',
      src:{type:'helm', repo:'github.com/acme/guestbook', rev:'main', chart:'guestbook', release:'guestbook-dev', values:['values.yaml', 'values-dev.yaml'], params:{'replicaCount':'1', 'image.tag':'latest'}}, res:std('guestbook-ui', 'dev', []).map((r, i) => i === 0 ? R_('Deployment', 'guestbook-ui', 'dev', 'OutOfSync', 'Healthy', 'image tag changed in Git: latest → sha-9f1c2') : r)},
    {name:'canary-rollouts', project:'platform', ns:'prod', set:null, last:8, auto:'manual', sync:'OutOfSync', health:'Healthy',
      src:{type:'kustomize', repo:'gitlab.com/acme/gitops', rev:'main', path:'apps/canary/overlays/prod', kver:'5.4.1', namePrefix:'prod-', images:['acme/canary-api=acme/canary-api:2.8.0'], labels:{'env':'prod', 'team':'platform'}},
      res:[R_('Rollout', 'canary-api', 'prod', 'OutOfSync', 'Healthy', 'Live replicas (4) differ from Git (3)'), R_('Service', 'canary-api-stable', 'prod'), R_('Service', 'canary-api-canary', 'prod', 'OutOfSync', 'Healthy', 'selector changed in Git'), R_('AnalysisTemplate', 'success-rate', 'prod'), R_('Ingress', 'canary-api', 'prod')]},
    {name:'prometheus-stack', project:'infra', ns:'monitoring', set:'infra-addons', last:1, auto:'auto-sync · self-heal', sync:'Synced', health:'Healthy',
      src:{type:'helm', repo:'github.com/acme/helm-charts', rev:'58.2.1', chart:'kube-prometheus-stack', release:'prometheus', values:['values.yaml'], params:{'grafana.enabled':'true', 'prometheus.retention':'15d'}},
      res:[R_('StatefulSet', 'prometheus-server', 'monitoring'), R_('Deployment', 'grafana', 'monitoring'), R_('DaemonSet', 'node-exporter', 'monitoring'), R_('Service', 'prometheus', 'monitoring'), R_('Service', 'grafana', 'monitoring'), R_('ConfigMap', 'prometheus-rules', 'monitoring'), R_('ServiceMonitor', 'kube-state-metrics', 'monitoring'), R_('PVC', 'prometheus-data', 'monitoring')]},
    {name:'cert-manager', project:'certs', ns:'security', set:'infra-addons', last:34, auto:'auto-sync', sync:'Synced', health:'Degraded',
      src:{type:'helm', repo:'github.com/cert-manager/cert-manager', rev:'1.15.0', chart:'cert-manager', release:'cert-manager', values:['values.yaml'], params:{'installCRDs':'true'}},
      res:[R_('Deployment', 'cert-manager', 'security'), R_('Deployment', 'cert-manager-webhook', 'security', 'Synced', 'Degraded', '0/1 pods ready: MountVolume.SetUp failed for volume "webhook-tls"'), R_('Deployment', 'cert-manager-cainjector', 'security'), R_('ServiceAccount', 'cert-manager', 'security'), R_('ClusterIssuer', 'letsencrypt-prod', '—'), R_('Certificate', 'wildcard-acme', 'security', 'Synced', 'Degraded', 'Issuing certificate as Secret does not exist')]},
    {name:'nginx-ingress', project:'edge', ns:'default', set:'infra-addons', last:12, auto:'auto-sync', sync:'Synced', health:'Progressing',
      src:{type:'kustomize', repo:'github.com/acme/infra', rev:'v1.10.1', path:'edge/nginx', kver:'5.4.1', namePrefix:'', images:['registry.k8s.io/ingress-nginx/controller:v1.10.1'], labels:{'app.kubernetes.io/part-of':'edge'}},
      res:[R_('Deployment', 'nginx-ingress-controller', 'default', 'Synced', 'Progressing', '2/3 replicas available'), R_('Service', 'nginx-ingress-controller', 'default'), R_('ConfigMap', 'nginx-config', 'default'), R_('IngressClass', 'nginx', '—')]},
    {name:'postgres-cluster', project:'data', ns:'data', set:'data-services', last:3, auto:'auto-sync · prune · self-heal', sync:'Synced', health:'Healthy',
      src:{type:'helm', repo:'github.com/acme/data-ops', rev:'12.8.0', chart:'postgresql', release:'postgres', values:['values.yaml', 'values-ha.yaml'], params:{'architecture':'replication', 'primary.persistence.size':'200Gi'}},
      res:[R_('StatefulSet', 'postgres', 'data'), R_('Service', 'postgres', 'data'), R_('Service', 'postgres-headless', 'data'), R_('Secret', 'postgres-credentials', 'data'), R_('PVC', 'data-postgres-0', 'data'), R_('PodDisruptionBudget', 'postgres', 'data')]},
    {name:'redis-cache', project:'data', ns:'data', set:'data-services', last:9, auto:'auto-sync', sync:'Synced', health:'Healthy',
      src:{type:'helm', repo:'github.com/acme/data-ops', rev:'19.5.0', chart:'redis', release:'redis', values:['values.yaml'], params:{'architecture':'standalone', 'master.persistence.size':'8Gi'}},
      res:[R_('StatefulSet', 'redis-master', 'data'), R_('Service', 'redis-master', 'data'), R_('ConfigMap', 'redis-config', 'data'), R_('PVC', 'redis-data-0', 'data')]},
    {name:'legacy-batch', project:'platform', ns:'batch', set:null, last:55, auto:'manual', sync:'Synced', health:'Healthy',
      src:{type:'dir', repo:'github.com/acme/legacy-jobs', rev:'release-2024', path:'manifests/', recurse:true},
      res:[R_('CronJob', 'nightly-export', 'batch'), R_('CronJob', 'cleanup-temp', 'batch'), R_('ConfigMap', 'batch-settings', 'batch'), R_('ServiceAccount', 'batch-runner', 'batch')]}
  ];
  const ASETS = [
    {name:'guestbook-envs', project:'demo-apps', gen:'List', genDesc:'3 elements: env = prod, staging, dev', repo:'github.com/acme/guestbook', srcType:'helm', tmpl:'guestbook-{{env}}', dest:'{{env}}', policy:'sync: automated (except dev) · preserveResourcesOnDeletion: false'},
    {name:'infra-addons', project:'infra', gen:'Cluster', genDesc:'clusters matching label env=prod (1 of 3 clusters)', repo:'github.com/acme/helm-charts', srcType:'helm', tmpl:'{{name}}', dest:'{{values.namespace}}', policy:'sync: automated · selfHeal: true'},
    {name:'data-services', project:'data', gen:'Git directory', genDesc:'github.com/acme/data-ops · path data/*', repo:'github.com/acme/data-ops', srcType:'helm', tmpl:'{{path.basename}}', dest:'data', policy:'sync: automated · prune: true'}
  ];
  const AUTHORS = ['dana.k', 'li.wei', 'sam.o', 'argocd-bot'], MSGS = ['bump image tag', 'tune resource limits', 'enable metrics scraping', 'rotate TLS secret', 'scale replicas', 'fix ingress host'];
  ARGO.forEach(a => {
    a.hist = [0, 1, 2, 3, 4].map(i => { const h = hashOf(a.name + i); return {rev:SHA(a.name + i), min:a.last + i * (40 + h % 300) + (i ? 20 : 0), by:AUTHORS[h % AUTHORS.length], msg:MSGS[h % MSGS.length], ok:!(i === 1 && a.name === 'nginx-ingress')}; });
    a.hist[0].ok = a.health !== 'Degraded';
    a.live = a.sync === 'Synced' ? a.hist[0].rev : a.hist[1].rev;
    // event stream: operations from the sync history plus whatever the live resources are complaining about
    a.events = [];
    a.hist.forEach((h, i) => a.events.push({type:h.ok ? 'Normal' : 'Warning', reason:h.ok ? 'OperationCompleted' : 'OperationFailed', msg:(h.ok ? 'Sync to ' : 'Sync failed at ') + h.rev + ' · ' + h.msg, min:h.min}));
    a.res.forEach(r => { if (r.msg) a.events.push({type:'Warning', reason:r.health === 'Degraded' ? 'ResourceDegraded' : r.sync !== 'Synced' ? 'ResourceOutOfSync' : 'ResourceProgressing', msg:r.kind + '/' + r.name + ': ' + r.msg, min:Math.max(1, a.last - 1)}); });
    a.events.push({type:'Normal', reason:'ResourceCreated', msg:a.res.length + ' resources created in ' + a.ns, min:a.hist[4].min + 120});
    a.events.sort((x, y) => x.min - y.min);
  });
  const A_SYNC_TONE = {Synced:'success', OutOfSync:'warn', Unknown:'warn'}, A_HEALTH_TONE = {Healthy:'success', Progressing:'warn', Suspended:'warn', Degraded:'danger', Missing:'danger'};
  const KIND_ICON = {Deployment:'deploy', Rollout:'deploy', StatefulSet:'storage', DaemonSet:'node', Service:'services', ConfigMap:'config', Secret:'config', Ingress:'network', IngressClass:'network', ServiceAccount:'projects', PVC:'storage', AnalysisTemplate:'check', ServiceMonitor:'logs', ClusterIssuer:'config', Certificate:'config', PodDisruptionBudget:'check', CronJob:'clock'};
  const AS = {tab:'apps', filter:'all', q:''};
  const agoMin = m => m < 1 ? 'just now' : m < 60 ? Math.round(m) + 'm ago' : m < 1440 ? Math.floor(m / 60) + 'h ago' : Math.floor(m / 1440) + 'd ago';
  const argoApp = n => ARGO.find(a => a.name === n), argoSet = n => ASETS.find(s => s.name === n);
  const setApps = s => ARGO.filter(a => a.set === s.name);
  const setState = s => { const as = setApps(s); return {sync:as.every(a => a.sync === 'Synced') ? 'Synced' : 'OutOfSync', health:as.some(a => a.health === 'Degraded') ? 'Degraded' : as.some(a => a.health === 'Progressing') ? 'Progressing' : 'Healthy', busy:as.some(a => a.busy) ? 'sync' : null, last:Math.min.apply(null, as.map(a => a.last).concat([9999]))}; };
  const aPill = (v, map) => pill(v, map[v] || 'warn');
  const srcBadge = (type, link) => '<span class="src-badge ' + type + '" title="Deployed with ' + SRC[type].label + '">' + ICON(SRC[type].icon) + SRC[type].label + '</span>';
  const srcTag = a => a.src.type === 'helm' ? a.src.chart + ' ' + a.src.rev : a.src.type === 'kustomize' ? a.src.path + ' @ ' + a.src.rev : a.src.path + ' @ ' + a.src.rev;
  const A_FILTERS = [['all', 'All', () => true], ['oos', 'Out of sync', x => x.sync !== 'Synced'], ['degraded', 'Degraded', x => x.health === 'Degraded'], ['progressing', 'Progressing', x => x.health === 'Progressing'], ['healthy', 'Healthy', x => x.sync === 'Synced' && x.health === 'Healthy']];
  const aPred = () => (A_FILTERS.find(f => f[0] === AS.filter) || A_FILTERS[0])[2];
  const aSearch = a => !AS.q.trim() || ((a.name || '') + ' ' + (a.project || '') + ' ' + (a.ns || '') + ' ' + (a.src ? a.src.repo + ' ' + SRC[a.src.type].label : '') + ' ' + (a.set || '') + ' ' + (a.gen || '')).toLowerCase().indexOf(AS.q.trim().toLowerCase()) >= 0;
  const aVisible = () => ARGO.filter(a => (!S.ns.size || S.ns.has(a.ns)) && aPred()(a) && aSearch(a));
  const sVisible = () => ASETS.filter(s => aPred()(Object.assign({}, s, setState(s))) && aSearch(s));
  const resBar = list => '<span class="res-bar" role="img" aria-label="' + list.length + ' items">' + list.map(r => '<i class="' + (r.health === 'Degraded' || r.health === 'Missing' ? 'danger' : r.health === 'Progressing' || r.sync !== 'Synced' ? 'warn' : 'ok') + '" title="' + esc((r.kind ? r.kind + ' ' : '') + r.name) + '"></i>').join('') + '</span>';
  const aStatusClass = a => a.health === 'Degraded' ? 'degraded' : a.sync !== 'Synced' ? 'outofsync' : a.health === 'Progressing' ? 'progressing' : 'synced';
  // icon-only sync / refresh: kind = 'app' | 'set'
  const caretBtn = (kind, n, busy) => '<button type="button" class="argo-caret" data-argo-rmenu="' + kind + '" data-rname="' + n + '"' + (busy ? ' disabled' : '') + ' aria-haspopup="menu" aria-expanded="false" aria-label="Refresh options for ' + n + '" title="Refresh options">' + ICON('chevron') + '</button>';
  const aIcons = (o, kind) => { const n = esc(o.name), busy = kind === 'set' ? setState(o).busy : o.busy;
    return '<span class="argo-icons">' +
      '<button type="button" class="argo-ic sync' + (busy === 'sync' ? ' spin' : '') + '" data-argo-' + (kind === 'set' ? 'setsync' : 'sync') + '="' + n + '"' + (busy ? ' disabled' : '') + ' aria-label="Sync ' + n + '" title="' + (busy === 'sync' ? 'Syncing…' : 'Sync') + '">' + ICON('sync') + '</button>' +
      '<span class="argo-split"><button type="button" class="argo-ic refresh' + (busy === 'refresh' ? ' spin' : '') + '" data-argo-' + (kind === 'set' ? 'setrefresh' : 'refresh') + '="' + n + '"' + (busy ? ' disabled' : '') + ' aria-label="Refresh ' + n + '" title="' + (busy === 'refresh' ? 'Refreshing…' : 'Refresh (soft)') + '">' + ICON('refresh') + '</button>' + caretBtn(kind, n, busy) + '</span></span>'; };
  const aBtns = (o, kind) => { const busy = kind === 'set' ? setState(o).busy : o.busy, n = esc(o.name), s = kind === 'set' ? 'setsync' : 'sync', r = kind === 'set' ? 'setrefresh' : 'refresh';
    return '<span class="argo-btns"><button type="button" class="btn btn-primary argo-btn" data-argo-' + s + '="' + n + '"' + (busy ? ' disabled' : '') + '>' + ICON('sync') + (busy === 'sync' ? 'Syncing…' : 'Sync') + '</button>' +
      '<span class="argo-split"><button type="button" class="btn btn-secondary argo-btn refresh' + (busy === 'refresh' ? ' spin' : '') + '" data-argo-' + r + '="' + n + '"' + (busy ? ' disabled' : '') + '>' + ICON('refresh') + (busy === 'refresh' ? 'Refreshing…' : 'Refresh') + '</button>' + caretBtn(kind, n, busy) + '</span></span>'; };
  function aCard(a){
    return '<article class="app-card static" data-app="' + esc(a.name) + '" data-argo-open="' + esc(a.name) + '" tabindex="0"><div class="card-main"><div class="card-status ' + aStatusClass(a) + '"></div><div class="card-body">' +
      '<div class="card-top">' + aIcons(a, 'app') + '<span class="card-top-r">' + srcBadge(a.src.type) + '</span></div>' +
      '<div class="card-title-row"><div><h2 class="app-name">' + esc(a.name) + '</h2><p class="app-sub">' + nsLink(a.ns) + ' / ' + esc(a.project) + '</p></div></div>' +
      '<div class="tag-row">' + (a.set ? '<button type="button" class="set-chip" data-argo-setopen="' + esc(a.set) + '" title="Generated by ApplicationSet ' + esc(a.set) + '">' + ICON('projects') + esc(a.set) + '</button>' : '') + '<span class="tag">' + esc(a.src.repo) + '</span><span class="tag">' + esc(srcTag(a)) + '</span></div>' +
      '<div class="res-line">' + resBar(a.res) + '<span class="res-count">' + cnt(a.res.length) + 'resources</span></div>' +
      '<div class="card-footer"><div class="badges">' + (a.busy === 'sync' ? pill('Syncing', 'warn') : aPill(a.sync, A_SYNC_TONE)) + aPill(a.health, A_HEALTH_TONE) + '</div><span class="card-meta">' + ICON('clock') + esc(a.last < 1 ? 'now' : a.last + 'm') + '</span></div>' +
      '</div></div></article>';
  }
  function sCard(s){
    const st = setState(s), as = setApps(s);
    return '<article class="app-card static set-card" data-set="' + esc(s.name) + '" data-argo-setopen="' + esc(s.name) + '" tabindex="0"><div class="card-main"><div class="card-status ' + aStatusClass(st) + '"></div><div class="card-body">' +
      '<div class="card-top">' + aIcons(s, 'set') + '<span class="card-top-r"><span class="src-badge set">' + ICON('projects') + 'ApplicationSet</span></span></div>' +
      '<div class="card-title-row"><div><h2 class="app-name">' + esc(s.name) + '</h2><p class="app-sub">' + esc(s.project) + ' · ' + esc(s.gen) + ' generator</p></div></div>' +
      '<div class="tag-row"><span class="tag">' + esc(s.genDesc) + '</span></div>' +
      '<div class="res-line">' + resBar(as.map(a => ({name:a.name, sync:a.sync, health:a.health}))) + '<span class="res-count">' + cnt(as.length) + 'applications</span></div>' +
      '<div class="card-footer"><div class="badges">' + (st.busy ? pill('Syncing', 'warn') : aPill(st.sync, A_SYNC_TONE)) + aPill(st.health, A_HEALTH_TONE) + '</div><span class="card-meta">' + srcBadge(s.srcType) + '</span></div>' +
      '</div></div></article>';
  }
  function aList(apps){
    return '<div class="table-wrap panel"><table class="resource-table argo-table"><thead><tr><th>Application</th><th>Source</th><th>Sync</th><th>Health</th><th>Repository</th><th>Resources</th><th>Synced</th><th class="col-act"><span class="sr-only">Actions</span></th></tr></thead><tbody>' +
      apps.map(a => '<tr tabindex="0" data-argo-open="' + esc(a.name) + '"><td data-label="Application" data-first><span class="cell-value row-first"><b>' + esc(a.name) + '</b><small class="muted"> ' + nsLink(a.ns) + ' / ' + esc(a.project) + '</small></span></td>' +
        '<td data-label="Source">' + srcBadge(a.src.type) + '</td><td data-label="Sync" data-mob>' + (a.busy === 'sync' ? pill('Syncing', 'warn') : aPill(a.sync, A_SYNC_TONE)) + '</td><td data-label="Health">' + aPill(a.health, A_HEALTH_TONE) + '</td><td data-label="Repository"><span class="mono">' + esc(a.src.repo) + '</span></td>' +
        '<td data-label="Resources"><span class="res-line">' + cnt(a.res.length) + resBar(a.res) + '</span></td><td data-label="Synced">' + esc(agoMin(a.last)) + '</td><td class="col-act argo-act" data-label="Actions">' + aIcons(a, 'app') + '</td></tr>').join('') + '</tbody></table></div>';
  }
  function aResources(apps){
    const pred = aPred(), groups = apps.map(a => ({a, rs:a.res.filter(r => AS.filter === 'all' || pred(r))})).filter(g => g.rs.length);
    return groups.map(g => '<section class="argo-group panel"><header class="argo-group-head">' + aIcons(g.a, 'app') + '<button type="button" class="link-btn argo-name" data-argo-open="' + esc(g.a.name) + '">' + esc(g.a.name) + '</button>' + cnt(g.rs.length) + srcBadge(g.a.src.type) + '<span class="badges">' + aPill(g.a.sync, A_SYNC_TONE) + aPill(g.a.health, A_HEALTH_TONE) + '</span></header>' +
      '<div class="table-wrap"><table class="resource-table argo-table"><tbody>' + g.rs.map(r => aResRow(g.a, r)).join('') + '</tbody></table></div></section>').join('');
  }
  const KIND_DATA = {Deployment:'deployments', StatefulSet:'statefulsets', DaemonSet:'daemonsets', CronJob:'cronjobs'};
  const aLive = r => { const k = KIND_DATA[r.kind]; const hit = k && DATA[k].find(x => x.name === r.name); return hit ? hit.id : null; };
  function aResRow(a, r){
    const live = aLive(r);
    return '<tr' + (live ? ' data-argo-goto="' + esc(live) + '" tabindex="0"' : ' data-argo-open="' + esc(a.name) + '" data-atab="resources" tabindex="0"') + '><td data-label="Kind" data-first><span class="cell-value row-first"><span class="kind-ic">' + ICON(KIND_ICON[r.kind] || 'config') + '</span><span class="kind-name">' + esc(r.kind) + '</span><b class="mono">' + esc(r.name) + '</b></span></td>' +
      '<td data-label="Namespace"><span class="mono">' + nsLink(r.ns) + '</span></td><td data-label="Sync" data-mob>' + aPill(r.sync, A_SYNC_TONE) + '</td><td data-label="Health">' + aPill(r.health, A_HEALTH_TONE) + '</td><td data-label="Message" class="argo-msg">' + esc(r.msg || '—') + '</td></tr>';
  }
  function aActivity(apps){
    const items = [];
    apps.forEach(a => a.hist.forEach(h => items.push({a, h})));
    items.sort((x, y) => x.h.min - y.h.min);
    return '<ol class="argo-timeline">' + items.slice(0, 24).map(x => '<li class="' + (x.h.ok ? 'ok' : 'bad') + '"><span class="tl-dot"></span><div class="tl-body"><div class="tl-head"><button type="button" class="link-btn argo-name" data-argo-open="' + esc(x.a.name) + '" data-atab="history">' + esc(x.a.name) + '</button><span class="mono sha">' + esc(x.h.rev) + '</span>' + pill(x.h.ok ? 'Succeeded' : 'Failed', x.h.ok ? 'success' : 'danger') + '<span class="tl-when">' + esc(agoMin(x.h.min)) + '</span></div><div class="tl-msg">' + esc(x.h.msg) + ' <small class="muted">by ' + esc(x.h.by) + '</small></div></div></li>').join('') + '</ol>';
  }
  function argoContent(){
    const box = $('#argoContent');
    if (!box) return;
    const apps = aVisible();
    let html = '';
    if (AS.tab === 'apps') html = apps.length ? '<div class="app-grid">' + apps.map(aCard).join('') + '</div>' : '';
    else if (AS.tab === 'sets') { const ss = sVisible(); html = ss.length ? '<div class="app-grid">' + ss.map(sCard).join('') + '</div>' : ''; }
    else if (AS.tab === 'list') html = apps.length ? aList(apps) : '';
    else if (AS.tab === 'resources') html = aResources(apps);
    else html = apps.length ? aActivity(apps) : '';
    box.innerHTML = html || '<div class="empty-state">No ' + (AS.tab === 'sets' ? 'ApplicationSets' : 'applications') + ' match the current filters.</div>';
  }
  function renderArgo(){
    const root = $('#argoRoot');
    if (!root) return;
    const oos = ARGO.filter(a => a.sync !== 'Synced').length, deg = ARGO.filter(a => a.health === 'Degraded').length;
    $('#argoSub').textContent = 'Sync and health across ' + ARGO.length + ' GitOps applications and ' + ASETS.length + ' ApplicationSets' + (oos ? ' · ' + oos + ' out of sync' : '') + (deg ? ' · ' + deg + ' degraded' : '') + '.';
    $('#argoSyncAll').innerHTML = ICON('sync') + 'Sync out-of-sync' + (oos ? cnt(oos) : '');
    $('#argoSyncAll').disabled = !oos;
    const tabs = [['apps', 'Applications', ARGO.length], ['sets', 'ApplicationSets', ASETS.length], ['list', 'List', ARGO.length], ['resources', 'Resources', ARGO.reduce((n, a) => n + a.res.length, 0)], ['activity', 'Activity', null]];
    const sc = root.querySelector('#argoSearch'), keep = sc && document.activeElement === sc;
    root.innerHTML = '<div class="argo-stats" role="group" aria-label="Filter by status">' + A_FILTERS.map(f => '<button type="button" class="chip' + (AS.filter === f[0] ? ' active' : '') + '" data-argo-filter="' + f[0] + '" aria-pressed="' + (AS.filter === f[0]) + '">' + f[1] + cnt(f[0] === 'all' ? ARGO.length : ARGO.filter(f[2]).length) + '</button>').join('') + '</div>' +
      '<div class="argo-bar"><div class="insp-tabs argo-tabs" role="tablist">' + tabs.map(t => '<button type="button" class="insp-tab" role="tab" data-argo-tab="' + t[0] + '" aria-selected="' + (AS.tab === t[0]) + '">' + t[1] + (t[2] != null ? cnt(t[2]) : '') + '</button>').join('') + '</div>' +
      '<label class="search-field argo-search"><svg class="icon"><use href="#i-search"/></svg><input type="search" id="argoSearch" placeholder="Filter by name, project, repo or source…" value="' + esc(AS.q) + '" aria-label="Filter applications" autocomplete="off"></label></div><div id="argoContent"></div>';
    argoContent();
    if (keep) { const i = $('#argoSearch'); i.focus(); i.setSelectionRange(i.value.length, i.value.length); }
  }
  /* actions */
  const argoDone = () => { renderArgo(); if (S.argoView) renderArgoDetail(); };
  const pushEv = (a, type, reason, msg) => { a.events.unshift({type, reason, msg, min:0}); };
  function syncApp(name, quiet){
    const a = argoApp(name);
    if (!a || a.busy) return;
    a.busy = 'sync'; pushEv(a, 'Normal', 'OperationStarted', 'Sync operation started (manual)'); argoDone();
    setTimeout(() => {
      a.busy = null; a.res.forEach(r => { r.sync = 'Synced'; if (r.health === 'Progressing') r.health = 'Healthy'; if (r.health === 'Healthy') r.msg = ''; });
      a.sync = 'Synced'; a.last = 0; a.live = SHA(a.name + Date.now());
      const stillBad = a.res.some(r => r.health === 'Degraded');
      a.health = stillBad ? 'Degraded' : 'Healthy';
      a.hist.unshift({rev:a.live, min:0, by:'you', msg:'manual sync', ok:!stillBad});
      pushEv(a, stillBad ? 'Warning' : 'Normal', stillBad ? 'OperationFailed' : 'OperationCompleted', stillBad ? 'Sync applied but ' + a.res.filter(r => r.health === 'Degraded').length + ' resource(s) are still Degraded' : 'Sync completed · ' + a.res.length + ' resources match Git');
      if (!quiet) showToast(stillBad ? a.name + ' synced, but still Degraded: fix the failing resources' : a.name + ' synced · all resources Healthy');
      argoDone();
    }, 900);
  }
  function refreshApp(name, quiet, hard){
    const a = argoApp(name);
    if (!a || a.busy) return;
    a.busy = 'refresh'; argoDone();
    setTimeout(() => {
      a.busy = null; a.last = 0;
      const n = a.res.filter(r => r.sync !== 'Synced').length;
      pushEv(a, 'Normal', hard ? 'HardRefresh' : 'ResourceRefreshed', (hard ? 'Manifest cache cleared and re-fetched from Git · ' : 'Compared with cached Git state · ') + (n ? n + ' resources differ' : 'no differences'));
      if (!quiet) showToast((hard ? 'Hard refresh · ' : '') + (a.sync === 'Synced' ? a.name + ' is up to date with Git' : a.name + ' differs from Git · ' + n + ' resources out of sync'));
      argoDone();
    }, hard ? 1500 : 600);
  }
  function syncSet(name){ const s = argoSet(name); if (!s) return; const as = setApps(s); as.forEach(a => syncApp(a.name, true)); showToast('Syncing ' + as.length + ' applications generated by ' + s.name); }
  function refreshSet(name, hard){ const s = argoSet(name); if (!s) return; const as = setApps(s); as.forEach(a => refreshApp(a.name, true, hard)); showToast((hard ? 'Hard refreshing ' : 'Refreshing ') + as.length + ' applications generated by ' + s.name); }
  /* details pane */
  function openArgoDetail(name, tab, kind){
    kind = kind || 'app';
    if (!(kind === 'set' ? argoSet(name) : argoApp(name))) return;
    if (!navKeep && inspector.classList.contains('open') && !(S.argoView && S.argoView.name === name && S.argoView.kind === kind)) navPush();
    const same = S.argoView && S.argoView.name === name && S.argoView.kind === kind;
    S.argoView = {name, kind, tab:tab || (same ? S.argoView.tab : 'summary')};
    S.metricView = null; S.inspect = null;
    $$('tr.selected', tbody).forEach(t => t.classList.remove('selected'));
    renderArgoDetail();
    inspector.classList.add('open'); inspectorScrim.classList.add('show');
    inspectorBody.scrollTop = 0;
  }
  const evList = list => '<ul class="ev-list argo-ev">' + (list.length ? list.slice(0, 30).map(e => '<li class="' + e.type.toLowerCase() + '"><span class="ev-type">' + esc(e.reason) + '</span><span class="ev-msg">' + esc(e.msg) + '</span><span class="ev-age">' + esc(e.min < 1 ? 'now' : e.min < 60 ? Math.round(e.min) + 'm' : e.min < 1440 ? Math.floor(e.min / 60) + 'h' : Math.floor(e.min / 1440) + 'd') + '</span></li>').join('') : '<li><span class="ev-msg">No events recorded.</span></li>') + '</ul>';
  const sourcePanel = a => {
    const s = a.src, rows = kv('Tool', srcBadge(s.type)) + kv('Repository', '<span class="mono">' + esc(s.repo) + '</span>') + kv('Target revision', '<span class="mono">' + esc(s.rev) + '</span>') + kv('Live revision', '<span class="mono">' + esc(a.live) + '</span>');
    if (s.type === 'helm') return '<dl class="kv-list">' + rows + kv('Chart', '<span class="mono">' + esc(s.chart) + ' ' + esc(s.rev) + '</span>') + kv('Release name', '<span class="mono">' + esc(s.release) + '</span>') + kv('Value files', chipsOf(s.values.reduce((o, v) => { o[v] = ''; return o; }, {}))) + '</dl>' +
      '<div class="inspector-eyebrow">Parameters</div>' + chipsOf(s.params);
    if (s.type === 'kustomize') return '<dl class="kv-list">' + rows + kv('Path', '<span class="mono">' + esc(s.path) + '</span>') + kv('Kustomize version', '<span class="mono">' + esc(s.kver) + '</span>') + kv('Name prefix', s.namePrefix ? '<span class="mono">' + esc(s.namePrefix) + '</span>' : '—') + '</dl>' +
      '<div class="inspector-eyebrow">Image overrides</div><ul class="plain-list">' + s.images.map(i => '<li class="mono">' + esc(i) + '</li>').join('') + '</ul><div class="inspector-eyebrow">Common labels</div>' + chipsOf(s.labels);
    return '<dl class="kv-list">' + rows + kv('Path', '<span class="mono">' + esc(s.path) + '</span>') + kv('Recurse', s.recurse ? 'yes' : 'no') + '</dl><p class="muted-note">Plain manifests applied as-is. No templating tool is configured.</p>';
  };
  function renderArgoDetail(){
    const v = S.argoView;
    if (!v) return closeInspector();
    if (v.kind === 'set') return renderSetDetail();
    const a = argoApp(v.name);
    if (!a) return closeInspector();
    $('#inspectorName').textContent = a.name; $('#copyName').hidden = false;
    $('#inspectorKind').innerHTML = 'Argo CD application · ' + nsLink(a.ns);
    const tabs = [['summary', 'Summary'], ['source', SRC[a.src.type].label], ['resources', 'Resources'], ['events', 'Events'], ['history', 'History']];
    if (!tabs.some(t => t[0] === v.tab)) v.tab = 'summary';
    let panel;
    if (v.tab === 'source') panel = sourcePanel(a);
    else if (v.tab === 'events') panel = evList(a.events);
    else if (v.tab === 'resources') {
      const kinds = {};
      a.res.forEach(r => { (kinds[r.kind] = kinds[r.kind] || []).push(r); });
      panel = Object.keys(kinds).map(k => '<div class="inspector-eyebrow">' + esc(k) + cnt(kinds[k].length) + '</div><ul class="argo-res-list">' + kinds[k].map(r => { const live = aLive(r);
        return '<li' + (live ? ' class="go" data-argo-goto="' + esc(live) + '" tabindex="0"' : '') + '><span class="kind-ic">' + ICON(KIND_ICON[r.kind] || 'config') + '</span><span class="argo-res-main"><b class="mono">' + esc(r.name) + '</b>' + (r.msg ? '<small>' + esc(r.msg) + '</small>' : '') + '</span><span class="badges">' + aPill(r.sync, A_SYNC_TONE) + aPill(r.health, A_HEALTH_TONE) + '</span></li>'; }).join('') + '</ul>').join('');
    } else if (v.tab === 'history') {
      panel = '<ol class="argo-timeline">' + a.hist.map((h, i) => '<li class="' + (h.ok ? 'ok' : 'bad') + '"><span class="tl-dot"></span><div class="tl-body"><div class="tl-head"><span class="mono sha">' + esc(h.rev) + '</span>' + pill(h.ok ? 'Succeeded' : 'Failed', h.ok ? 'success' : 'danger') + (i === 0 ? '<span class="tag">current</span>' : '') + '<span class="tl-when">' + esc(agoMin(h.min)) + '</span></div><div class="tl-msg">' + esc(h.msg) + ' <small class="muted">by ' + esc(h.by) + '</small></div></div></li>').join('') + '</ol>';
    } else {
      const bad = a.res.filter(r => r.sync !== 'Synced' || r.health !== 'Healthy'), kc = {};
      a.res.forEach(r => { kc[r.kind] = (kc[r.kind] || 0) + 1; });
      panel = '<div class="argo-hero">' + srcBadge(a.src.type) + (a.busy === 'sync' ? pill('Syncing', 'warn') : aPill(a.sync, A_SYNC_TONE)) + aPill(a.health, A_HEALTH_TONE) + '<span class="muted">' + esc(a.last < 1 ? 'synced just now' : 'synced ' + agoMin(a.last)) + '</span></div>' +
        (bad.length ? '<div class="argo-issues"><b>' + cnt(bad.length) + (bad.length === 1 ? 'resource needs attention' : 'resources need attention') + '</b><ul>' + bad.map(r => '<li><span class="mono">' + esc(r.kind + '/' + r.name) + '</span> <small>' + esc(r.msg || (r.sync + ' · ' + r.health)) + '</small></li>').join('') + '</ul></div>' : '<div class="argo-issues ok"><b>Everything matches Git and is healthy.</b></div>') +
        '<dl class="kv-list">' + kv('Project', esc(a.project)) + kv('Destination', '<span class="mono">in-cluster / </span>' + nsLink(a.ns)) + kv('Source', srcBadge(a.src.type) + ' <span class="mono">' + esc(srcTag(a)) + '</span> <button type="button" class="link-btn" data-argo-dtab="source">details →</button>') +
        (a.set ? kv('Generated by', '<button type="button" class="link-btn" data-argo-setopen="' + esc(a.set) + '">' + esc(a.set) + ' (ApplicationSet) →</button>') : kv('Managed by', 'directly (not from an ApplicationSet)')) + kv('Sync policy', esc(a.auto)) + '</dl>' +
        '<div class="inspector-eyebrow">Deployed resources</div><div class="res-line">' + resBar(a.res) + '</div><div class="chip-list">' + Object.keys(kc).map(k => '<span class="label-chip"><b>' + esc(k) + '</b>' + cnt(kc[k]) + '</span>').join('') + '</div>';
    }
    setInspMenu([{toast:'Opening ' + a.name + ' in the Argo CD UI', label:'Open in Argo CD', icon:'arrow'}]);
    inspectorBody.innerHTML = '<div class="qa-row argo-qa">' + aBtns(a, 'app') + '</div>' +
      '<div class="insp-tabs" role="tablist">' + tabs.map(t => '<button type="button" class="insp-tab" role="tab" data-argo-dtab="' + t[0] + '" aria-selected="' + (v.tab === t[0]) + '">' + t[1] + (t[0] === 'resources' ? cnt(a.res.length) : '') + '</button>').join('') + '</div><div class="insp-panel" role="tabpanel">' + panel + '</div>';
  }
  function renderSetDetail(){
    const v = S.argoView, s = argoSet(v.name);
    if (!s) return closeInspector();
    const as = setApps(s), st = setState(s);
    $('#inspectorName').textContent = s.name; $('#copyName').hidden = false;
    $('#inspectorKind').textContent = 'ApplicationSet · ' + s.project;
    const tabs = [['summary', 'Summary'], ['apps', 'Applications'], ['events', 'Events']];
    if (!tabs.some(t => t[0] === v.tab)) v.tab = 'summary';
    let panel;
    if (v.tab === 'apps') panel = '<ul class="argo-res-list">' + as.map(a => '<li class="go" data-argo-open="' + esc(a.name) + '" tabindex="0"><span class="argo-res-main"><b class="mono">' + esc(a.name) + '</b><small>' + esc(a.ns) + ' · ' + esc(srcTag(a)) + '</small></span><span class="badges">' + aPill(a.sync, A_SYNC_TONE) + aPill(a.health, A_HEALTH_TONE) + '</span></li>').join('') + '</ul>';
    else if (v.tab === 'events') {
      const evs = [{type:'Normal', reason:'ApplicationsGenerated', msg:as.length + ' applications generated from the ' + s.gen + ' generator', min:240}];
      as.forEach(a => a.events.filter(e => e.type === 'Warning' || e.reason === 'OperationCompleted').slice(0, 2).forEach(e => evs.push({type:e.type, reason:e.reason, msg:a.name + ': ' + e.msg, min:e.min})));
      panel = evList(evs.sort((x, y) => x.min - y.min));
    } else panel = '<div class="argo-hero">' + srcBadge(s.srcType) + (st.busy ? pill('Syncing', 'warn') : aPill(st.sync, A_SYNC_TONE)) + aPill(st.health, A_HEALTH_TONE) + '<span class="muted">' + as.length + ' applications</span></div>' +
      '<dl class="kv-list">' + kv('Project', esc(s.project)) + kv('Generator', '<b>' + esc(s.gen) + '</b>') + kv('Generator input', esc(s.genDesc)) + kv('Template name', '<span class="mono">' + esc(s.tmpl) + '</span>') + kv('Template source', srcBadge(s.srcType) + ' <span class="mono">' + esc(s.repo) + '</span>') + kv('Destination namespace', '<span class="mono">' + esc(s.dest) + '</span>') + kv('Policy', esc(s.policy)) + '</dl>' +
      '<div class="inspector-eyebrow">Generated applications' + cnt(as.length) + '</div><div class="res-line">' + resBar(as.map(a => ({name:a.name, sync:a.sync, health:a.health}))) + '</div>' +
      '<ul class="argo-res-list">' + as.map(a => '<li class="go" data-argo-open="' + esc(a.name) + '" tabindex="0"><span class="argo-res-main"><b class="mono">' + esc(a.name) + '</b></span><span class="badges">' + aPill(a.sync, A_SYNC_TONE) + aPill(a.health, A_HEALTH_TONE) + '</span></li>').join('') + '</ul>';
    setInspMenu([{toast:'Opening ' + s.name + ' in the Argo CD UI', label:'Open in Argo CD', icon:'arrow'}]);
    inspectorBody.innerHTML = '<div class="qa-row argo-qa">' + aBtns(s, 'set') + '</div>' +
      '<div class="insp-tabs" role="tablist">' + tabs.map(t => '<button type="button" class="insp-tab" role="tab" data-argo-dtab="' + t[0] + '" aria-selected="' + (v.tab === t[0]) + '">' + t[1] + (t[0] === 'apps' ? cnt(as.length) : '') + '</button>').join('') + '</div><div class="insp-panel" role="tabpanel">' + panel + '</div>';
  }
  /* refresh menu: soft (default) or hard */
  const refMenu = $('#refMenu');
  let refCtx = null;
  function openRefMenu(btn){
    closePops();
    refCtx = {kind:btn.dataset.argoRmenu, name:btn.dataset.rname};
    refMenu.innerHTML = '<button type="button" class="rm-item ref-opt" role="menuitem" data-ref="soft"><span class="ref-ic soft">' + ICON('refresh') + '</span><span class="rm-text"><b>Soft refresh</b><small>Compare with the cached Git state. Default.</small></span></button>' +
      '<button type="button" class="rm-item ref-opt" role="menuitem" data-ref="hard"><span class="ref-ic hard">' + ICON('refresh') + '</span><span class="rm-text"><b>Hard refresh</b><small>Clear the manifest cache and re-fetch from Git.</small></span></button>';
    placePop(refMenu, btn, false); btn.setAttribute('aria-expanded', 'true');
  }
  refMenu.addEventListener('click', e => {
    const o = e.target.closest('[data-ref]');
    if (!o || !refCtx) return;
    const hard = o.dataset.ref === 'hard', c = refCtx;
    closePops();
    if (c.kind === 'all') { ARGO.forEach(a => refreshApp(a.name, true, hard)); showToast((hard ? 'Hard refreshing all ' : 'Refreshing all ') + ARGO.length + ' applications'); }
    else if (c.kind === 'set') refreshSet(c.name, hard);
    else refreshApp(c.name, false, hard);
  });
  let edRaf = 0;
  inspectorBody.addEventListener('input', e => {
    if (e.target.id !== 'yamlEdit' || !S.edit) return;
    S.edit.text = e.target.value;
    if (!edRaf) edRaf = requestAnimationFrame(() => { edRaf = 0; refreshEditor(); });
  });
  inspectorBody.addEventListener('scroll', e => { if (e.target.id === 'yamlEdit') syncEditorScroll(); }, true);
  inspectorBody.addEventListener('click', e => {
    const li = e.target.closest && e.target.closest('#yamlIssues li[data-yl]');
    if (!li) return;
    const ta = $('#yamlEdit'), ln = Number(li.dataset.yl), lines = ta.value.split('\n');
    let pos = 0; for (let k = 0; k < ln - 1; k++) pos += lines[k].length + 1;
    ta.focus(); ta.setSelectionRange(pos + Math.max(0, Number(li.dataset.yc) - 1), pos + lines[ln - 1].length);
    ta.scrollTop = Math.max(0, (ln - 4) * 18.6); syncEditorScroll();
  });
  inspectorBody.addEventListener('keydown', e => {
    if (e.target.id !== 'yamlEdit') return;
    if (e.key === 'Escape') { e.stopPropagation(); e.preventDefault(); const id = S.edit && S.edit.id; S.edit = null; if (id) renderInspector(id); }
    else if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) { e.preventDefault(); applyEdit(); }
    else if (e.key === 'Tab') { e.preventDefault(); const t = e.target, p = t.selectionStart; t.setRangeText('  ', p, t.selectionEnd, 'end'); t.dispatchEvent(new Event('input', {bubbles:true})); }
  });
  /* header ⋮ menu */
  const inspMenu = $('#inspMenu');
  $('#inspMore').addEventListener('click', e => {
    if (!inspMenu.hidden) return void closePops();
    closePops();
    inspMenu.innerHTML = inspMenuItems.map((m, i) => '<button type="button" role="menuitem" class="rm-item' + (m.danger ? ' danger' : '') + '" data-im="' + i + '">' + ICON(m.icon) + esc(m.label) + '</button>').join('');
    placePop(inspMenu, e.currentTarget, true); e.currentTarget.setAttribute('aria-expanded', 'true');
    const f = $('.rm-item', inspMenu); if (f) f.focus();
  });
  inspMenu.addEventListener('click', e => {
    const b = e.target.closest('[data-im]');
    if (!b) return;
    const m = inspMenuItems[Number(b.dataset.im)];
    closePops();
    if (!m) return;
    if (m.toast) showToast(m.toast);
    else if (S.inspect) runAction(m.act, S.inspect);
  });
  /* events */
  new MutationObserver(navUI).observe(inspector, {attributes:true, attributeFilter:['class']});
  $('#copyName').addEventListener('click', () => { const n = $('#inspectorName').textContent; copyText(n, 'Copied ' + n); });
  $('#navBack').addEventListener('click', navBack);
  $('#inspBack').addEventListener('click', navBack);
  document.addEventListener('keydown', e => { if (e.altKey && e.key === 'ArrowLeft' && NAV_STACK.length) { e.preventDefault(); navBack(); } });
  const argoView = $('#view-argocd');
  const argoClick = e => {
    const t = e.target;
    if (t.closest('[data-ns-filter]')) return true;   // handled globally; do not open the app
    let b = t.closest('[data-argo-rmenu]'); if (b) { e.stopPropagation(); if (!refMenu.hidden) closePops(); else openRefMenu(b); return true; }
    b = t.closest('[data-argo-sync]'); if (b) { e.stopPropagation(); syncApp(b.dataset.argoSync); return true; }
    b = t.closest('[data-argo-refresh]'); if (b) { e.stopPropagation(); refreshApp(b.dataset.argoRefresh); return true; }
    b = t.closest('[data-argo-setsync]'); if (b) { e.stopPropagation(); syncSet(b.dataset.argoSetsync); return true; }
    b = t.closest('[data-argo-setrefresh]'); if (b) { e.stopPropagation(); refreshSet(b.dataset.argoSetrefresh); return true; }
    b = t.closest('[data-argo-toast]'); if (b) { showToast(b.dataset.argoToast); return true; }
    b = t.closest('[data-argo-dtab]'); if (b && S.argoView) { S.argoView.tab = b.dataset.argoDtab; renderArgoDetail(); return true; }
    b = t.closest('[data-argo-goto]'); if (b) { gotoResource(b.dataset.argoGoto); return true; }
    b = t.closest('[data-argo-setopen]'); if (b) { e.stopPropagation(); openArgoDetail(b.dataset.argoSetopen, undefined, 'set'); return true; }
    if (S.argoView) { b = t.closest('[data-argo-open]'); if (b && inspector.contains(b)) { openArgoDetail(b.dataset.argoOpen, b.dataset.atab); return true; } }
    return false;
  };
  inspectorBody.addEventListener('click', e => { if (S.argoView && argoClick(e)) e.stopImmediatePropagation(); }, true);
  argoView.addEventListener('click', e => {
    if (argoClick(e)) return;
    const f = e.target.closest('[data-argo-filter]'); if (f) { AS.filter = f.dataset.argoFilter; renderArgo(); return; }
    const tb = e.target.closest('[data-argo-tab]'); if (tb) { AS.tab = tb.dataset.argoTab; renderArgo(); return; }
    const o = e.target.closest('[data-argo-open]'); if (o) openArgoDetail(o.dataset.argoOpen, o.dataset.atab);
  });
  argoView.addEventListener('keydown', e => {
    const o = e.target.closest && e.target.closest('[data-argo-open],[data-argo-goto],[data-argo-setopen]');
    if (o && e.target === o && (e.key === 'Enter' || e.key === ' ')) { e.preventDefault(); o.click(); }
  });
  inspectorBody.addEventListener('keydown', e => {
    const o = S.argoView && e.target.closest && e.target.closest('[data-argo-open],[data-argo-goto]');
    if (o && e.target === o && (e.key === 'Enter' || e.key === ' ')) { e.preventDefault(); o.click(); }
  });
  argoView.addEventListener('input', e => { if (e.target.id === 'argoSearch') { AS.q = e.target.value; argoContent(); } });
  $('#argoRefreshAll').addEventListener('click', () => { ARGO.forEach(a => refreshApp(a.name, true)); showToast('Refreshing all ' + ARGO.length + ' applications'); });
  $('#argoSyncAll').addEventListener('click', () => { const t = ARGO.filter(a => a.sync !== 'Synced'); t.forEach(a => syncApp(a.name, true)); if (t.length) showToast('Syncing ' + t.length + ' out-of-sync application' + (t.length === 1 ? '' : 's')); });
  $('#argoNew').addEventListener('click', () => showToast('New Application flow opens here'));

  /* ---- icon-only buttons: one rule for the whole app. Any .btn / .qa that carries an icon drops its written label visually
     (the label stays as tooltip + accessible name and keeps updating if the text changes). Dialog buttons keep their words. */
  const IC_SKIP = '#dlgScrim, .search-sheet, .menu-pop, .row-menu, #settingsPanel, .no-ic';
  function iconifyAll(root){
    (root || document).querySelectorAll('button.btn, button.qa').forEach(b => {
      if (!b.querySelector(':scope > svg.icon, :scope > .icon') || b.closest(IC_SKIP)) return;
      let label = '', badge = '';
      b.childNodes.forEach(n => {
        if (n.nodeType === 3) label += n.textContent;
        else if (n.nodeType === 1 && n.classList.contains('cnt')) badge = n.textContent;
        else if (n.nodeType === 1 && !n.classList.contains('icon') && n.tagName !== 'svg') label += n.textContent;
      });
      label = label.replace(/\s+/g, ' ').trim();
      if (!label) return;
      const full = label + (badge ? ' (' + badge + ')' : '');
      if (!b.classList.contains('ic')) b.classList.add('ic');
      if (b.getAttribute('aria-label') !== full) { b.setAttribute('aria-label', full); b.title = full; }
    });
  }
  let icRaf = 0;
  new MutationObserver(() => { if (!icRaf) icRaf = requestAnimationFrame(() => { icRaf = 0; iconifyAll(); }); }).observe(document.body, {childList:true, subtree:true, characterData:true});
  iconifyAll();

  document.addEventListener('keydown', e => {
    if (e.key !== 'Escape') return;
    if (!dlgScrim.hidden) return closeDlg();
    if (S.sr.open || sheet.classList.contains('pulling')) { closeSearch(); return; }
    if (closePops()) return;
    if (!addMenu.hidden) return closeAddMenu();
    if (!rowMenu.hidden) return closeRowMenu();
    if (!menuPop.hidden) return closeMenu();
    closeSidebar(); closeInspector();
  });

  /* ---------------------------------------------------------------- boot */
  applyTheme(document.documentElement.dataset.theme === 'dark' ? 'dark' : 'light', false);
  applyCluster();
  updateNavControls();
  setView('workloads', 'pods');
  showTab('term:cluster');
})();
