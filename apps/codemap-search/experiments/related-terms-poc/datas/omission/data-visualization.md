# Charts and data visualization 누락 단어

- 분야 id: `data-visualization`
- 원본: [../data-visualization.md](../data-visualization.md)
- 누락 단어 29개 · 사전에 없음 17 · 뜻 미배정 0 · 다른 분야에만 있음 12
- 기준: 데이터셋에 없거나, 분야 없이 있거나, 다른 분야에만 있는 단어만 남겼습니다. 다른 단어의 관련어로 이미 들어 있는 단어는 뺐습니다.

## 기존 주제에 넣을 단어

### Categorical chart scales (`t279.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `quantize scale` | phrase | 사전에 없음 | 연속 값을 이산 구간으로 나누는 스케일 개념으로 D3와 Vega 문서에 쓰입니다. |

### Geographic chart interaction (`t101.3`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `st.map` | identifier | 사전에 없음 | Streamlit 대시보드에서 지도를 그리는 함수입니다. |

### Geospatial web maps (`t492`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `TopoJSON` | proper_noun | 다른 분야에만 있음: data-formats — JSON format parsing | GeoJSON을 위상 정보로 압축하는 형식으로 D3 지도 코드에서 지도 데이터에 쓰입니다. |

### Statistical plotting (`t956`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `density plot` | phrase | 사전에 없음 | 분포의 밀도 곡선을 그리는 그림으로 Seaborn, ggplot 문서에 쓰입니다. |
| `sns.kdeplot` | identifier | 사전에 없음 | Seaborn에서 커널 밀도 곡선을 그리는 함수입니다. |
| `sns.pairplot` | identifier | 사전에 없음 | Seaborn에서 변수 쌍 산점도 격자를 그리는 함수입니다. |
| `swarm plot` | phrase | 사전에 없음 | 겹치지 않게 점을 흩뿌려 분포를 보여주는 Seaborn 그림 이름입니다. |
| `violin plot` | phrase | 다른 분야에만 있음: observability-debugging — Distribution plots | 분포 형태를 보여주는 차트 종류로 Seaborn, Plotly 시각화 코드에 쓰입니다. |

## 새 주제가 필요한 단어

### Additional chart types

범위: Less common chart types such as waterfall, parallel coordinates, contour, and word cloud charts.

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `contour plot` | phrase | 사전에 없음 | 값이 같은 지점을 잇는 등고선 그림으로 Matplotlib, Plotly 문서에 쓰입니다. |
| `lollipop chart` | phrase | 사전에 없음 | 막대 대신 선과 점으로 값을 보여주는 차트 종류입니다. |
| `parallel coordinates` | phrase | 사전에 없음 | 여러 변수를 평행한 축 위의 선으로 비교하는 차트로 Plotly, D3 문서에 쓰입니다. |
| `pareto chart` | phrase | 사전에 없음 | 막대와 누적 비율 선을 함께 보여주는 차트로 품질·원인 분석 문서에 쓰입니다. |
| `surface plot` | phrase | 사전에 없음 | 3차원 곡면을 그리는 그림으로 Matplotlib, Plotly 문서에 쓰입니다. |
| `venn diagram` | phrase | 사전에 없음 | 집합 겹침을 보여주는 다이어그램으로 시각화 라이브러리 문서에 쓰입니다. |
| `waterfall chart` | phrase | 다른 분야에만 있음: math-numerics — Counter increments | 범위에 명시된 폭포 차트로 이 분야에서는 증감 누적을 보여주는 차트 종류입니다. |
| `word cloud` | phrase | 다른 분야에만 있음: databases-storage — Text features | 단어 빈도를 글자 크기로 보여주는 차트로 이 분야에서는 시각화 종류입니다. |

### BI and dashboard tools

범위: Business intelligence and dashboard products and their workbook files for building data reports.

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Looker` | proper_noun | 다른 분야에만 있음: observability-debugging — BI fact measures | BI 보고서와 대시보드를 만드는 도구로 LookML 코드와 연동 설정에 쓰입니다. |
| `Metabase` | proper_noun | 다른 분야에만 있음: observability-debugging — BI fact measures | SQL 질의를 차트와 대시보드로 보여주는 오픈소스 BI 도구입니다. |
| `.pbix` | extension | 다른 분야에만 있음: data-engineering — Power BI reports | Power BI 보고서 파일 확장자로 BI 저장소와 배포 스크립트에 쓰입니다. |
| `Power BI` | proper_noun | 다른 분야에만 있음: auth-identity — Tenant organization isolation | 범위에 명시된 BI 도구로 이 분야에서는 보고서와 시각화 제작 도구로 쓰입니다. |
| `Qlik` | proper_noun | 사전에 없음 | BI 분석과 대시보드를 만드는 제품 이름입니다. |
| `QuickSight` | proper_noun | 사전에 없음 | AWS의 BI 대시보드 서비스 이름입니다. |
| `Tableau` | proper_noun | 다른 분야에만 있음: observability-debugging — Visualization dashboards | 범위에 명시된 BI 도구로 이 분야에서는 차트와 대시보드 제작 도구로 쓰입니다. |
| `.twb` | extension | 다른 분야에만 있음: general-wording — Tableau workbooks | Tableau 통합 문서 파일 확장자입니다. |

### Charting libraries

범위: Charting and plotting libraries not covered by a more specific topic.

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `amCharts` | proper_noun | 사전에 없음 | 웹 차트와 지도를 그리는 자바스크립트 차트 라이브러리입니다. |
| `FusionCharts` | proper_noun | 사전에 없음 | 웹 대시보드용 상용 차트 라이브러리입니다. |
| `Google Charts` | proper_noun | 사전에 없음 | 웹 페이지에 차트를 넣는 Google의 자바스크립트 차트 라이브러리입니다. |
| `uPlot` | proper_noun | 다른 분야에만 있음: software-design — Plugin lifecycle | 시계열 차트에 특화된 작고 빠른 캔버스 차트 라이브러리로 이 분야에서는 차트 라이브러리 이름입니다. |
| `Victory` | proper_noun | 다른 분야에만 있음: mobile-desktop — React Native interfaces | React와 React Native용 차트 컴포넌트 라이브러리로 이 분야에서는 차트 라이브러리 이름입니다. |
