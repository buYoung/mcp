export interface BenchmarkSeries {
    id: string;
    label: string;
    color: string;
    dash: string;
    width: number;
    primary: boolean;
    acc: number[];
    accDetail: string[];
    tok: number[];
    tokDetail: string[];
}

export interface BenchmarkPayload {
    envs: string[];
    series: BenchmarkSeries[];
    xCoords: number[];
    padTop: number;
    plotH: number;
    padLeft: number;
    svgW: number;
}
