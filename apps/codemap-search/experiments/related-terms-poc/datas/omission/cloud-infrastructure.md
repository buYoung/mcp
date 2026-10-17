# Cloud, containers and infrastructure 누락 단어

- 분야 id: `cloud-infrastructure`
- 원본: [../cloud-infrastructure.md](../cloud-infrastructure.md)
- 누락 단어 19개 · 사전에 없음 14 · 뜻 미배정 0 · 다른 분야에만 있음 5
- 기준: 데이터셋에 없거나, 분야 없이 있거나, 다른 분야에만 있는 단어만 남겼습니다. 다른 단어의 관련어로 이미 들어 있는 단어는 뺐습니다.

## 기존 주제에 넣을 단어

### Container image builds (`t383`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `docker buildx bake` | phrase | 사전에 없음 | 여러 이미지 빌드를 파일로 선언해 실행하는 buildx 명령이다. |

### Firebase application services (`t174.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Firebase CLI` | proper_noun | 사전에 없음 | firebase 명령으로 배포와 에뮬레이터를 실행하는 도구다. |

### Infrastructure compute services (`t670.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Vultr` | proper_noun | 사전에 없음 | VPS 클라우드 공급자로 Terraform provider에 쓰인다. |

### Kubernetes control plane (`t720.2`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `CoreDNS` | proper_noun | 다른 분야에만 있음: distributed-systems — Service discovery | 쿠버네티스 클러스터 DNS를 제공하는 기본 애드온으로 ConfigMap과 문서에 쓰인다. |
| `etcd` | proper_noun | 다른 분야에만 있음: configuration — Remote configuration stores | 쿠버네티스 클러스터 상태를 저장하는 키-값 저장소로 control plane 운영 문서에 쓰인다. |

### Kubernetes runtime integration (`t245.5`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Cluster API` | proper_noun | 사전에 없음 | 쿠버네티스 클러스터를 선언적으로 만들고 관리하는 프로젝트다. |
| `kops` | proper_noun | 사전에 없음 | AWS에서 쿠버네티스 클러스터를 프로비저닝하는 도구다. |
| `kubespray` | proper_noun | 사전에 없음 | Ansible로 쿠버네티스를 설치하는 도구다. |
| `Talos` | proper_noun | 사전에 없음 | 쿠버네티스 전용 불변 리눅스 운영체제다. |

### Managed relational databases (`t670.3`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Cloud Spanner` | proper_noun | 사전에 없음 | GCP 관리형 분산 관계형 데이터베이스다. |

### Microsoft Azure (`t109`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Application Gateway` | proper_noun | 사전에 없음 | Azure L7 로드 밸런서로 Bicep과 Terraform에 쓰인다. |
| `Azure Container Instances` | proper_noun | 사전에 없음 | 서버 관리 없이 컨테이너를 실행하는 Azure 서비스다. |

### OCI container runtimes (`t245.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Colima` | proper_noun | 사전에 없음 | macOS에서 컨테이너 런타임을 제공하는 도구다. |

### Terraform infrastructure state (`t648.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `CloudFormation stack` | phrase | 사전에 없음 | CloudFormation이 배포한 리소스 묶음 단위다. |
| `null_resource` | identifier | 사전에 없음 | Terraform에서 provisioner를 붙이는 더미 리소스다. |

## 새 주제가 필요한 단어

### kubectl cluster operations

범위: Operating Kubernetes clusters from the command line: kubectl commands, kubeconfig contexts, port forwarding, and log or exec access.

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `k9s` | proper_noun | 사전에 없음 | 터미널에서 클러스터를 탐색하는 TUI 도구다. |

### Kubernetes policy and access control

범위: Kubernetes admission control, RBAC roles and bindings, policy engines (OPA/Gatekeeper, Kyverno), and Pod security settings.

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Open Policy Agent` | proper_noun | 다른 분야에만 있음: auth-identity — Authorization policies | 쿠버네티스 admission 정책을 Rego로 작성하는 정책 엔진이다. |
| `RBAC` | abbreviation | 다른 분야에만 있음: auth-identity — Role assignments | 쿠버네티스 역할 기반 접근 제어로 Role과 ClusterRole 매니페스트에 쓰인다. |
| `securityContext` | identifier | 다른 분야에만 있음: auth-identity — Spring Security | Pod와 컨테이너의 보안 실행 옵션 필드다. |
