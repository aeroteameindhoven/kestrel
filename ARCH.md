# kestrel

## Remote deployment of kestrel-serial-agent

```mermaid
graph LR
    subgraph Monitoring Server
        Grafana
        Loki[Grafana Loki]
        Mimir[Grafana Mimir]
        Tempo[Grafana Tempo]
        NodeExporterS[
            Prometheus
            Node-Exporter
        ]
        TailscaleS[Tailscale]

        Loki --> Grafana
        Mimir --> Grafana
        Tempo --> Grafana

        NodeExporterS --> Mimir
    end

    subgraph Stand
        subgraph Battery Swap Micro-Controller
            Demo[Demo Sequence]
            Telemetry
            Command[Command Dispatcher]

            Demo --> Telemetry
            Command --> Demo
        end

        subgraph Battery Swap Monitor
            SerialAgent[Kestrel Serial Agent]
            NodeExporter[
                Prometheus
                Node-Exporter
            ]
            Promtail
            TailscaleM[Tailscale]

            Telemetry --UART--> SerialAgent
            SerialAgent --UART--> Command

            NodeExporter -- Mimir --> TailscaleM --Wireguard--> TailscaleS --> Mimir
            Promtail -- Loki --> TailscaleM; TailscaleS --> Loki
            SerialAgent -- Tempo --> TailscaleM; TailscaleS --> Tempo
        end
    end

    subgraph Developer
        Kestrel[Kestrel Client]
        TailscaleK[Tailscale]

        Kestrel --WebSocket--> TailscaleK --Wireguard--> TailscaleM --Websocket--> SerialAgent
    end

    subgraph Sysadmin
        Browser[Sysadmin's Browser]
        TailscaleB[Tailscale]

        Browser --HTTP--> TailscaleB --Wireguard--> TailscaleS --HTTP--> Grafana
    end

```

| Service                  | Units                            | Function       |
| ------------------------ | -------------------------------- | -------------- |
| Kestrel Serial Agent     | kestrel.service, kestrel.socket? | Serial Daemon? |
| Prometheus Node-Exporter | ...                              | Telemetry      |
| Tailscale                | ...                              | Connectivity   |

### Aggregate Server

| Service       | Units | Function        |
| ------------- | ----- | --------------- |
| Grafana Loki  | ...   | Log Aggregation |
| Grafana Tempo | ...   | Tracing Backend |
| Grafana Mimir | ...   | Metric Backend  |

### Kestrel Client

| S              |
| -------------- |
| Kestrel Client |
| Tailscale      |
