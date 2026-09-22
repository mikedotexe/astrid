    fn context<'a>(
        db: &'a BridgeDb,
        sensory_tx: &'a mpsc::Sender<crate::types::SensoryMsg>,
        telemetry: &'a SpectralTelemetry,
        response_text: &'a str,
        burst_count: &'a mut u32,
    ) -> NextActionContext<'a> {
        NextActionContext {
            operation_id: None,
            burst_count,
            db,
            sensory_tx,
            telemetry,
            fill_pct: telemetry.fill_pct(),
            response_text,
            workspace: Some(test_workspace_dir()),
        }
    }
