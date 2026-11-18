use crate::trace_support as trace;
trace::component!(trace::Component {
    description: "realtime-gateway-server コンポーネント",
    notes: &[],
    external: &[],
    operations: &[],
    entrypoints: &[trace::EntryPoint {
        name: "リアルタイムゲートウェイ起動",
        source: trace::Source {
            path: "apps/realtime-gateway/src/main.rs",
            needle: "async fn main() -> Result<(), Box<dyn std::error::Error>> {"
        }
    }]
});
