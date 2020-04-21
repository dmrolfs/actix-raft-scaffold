trait ChangeClusterBehavior {
    fn change_cluster_config(
        &self,
        to_add: Vec<NodeId>,
        to_remove: Vec<NodeId>,
        ctx: &<Node as Actor>::Context
    ) -> Result<(), NodeError>;
}

trait ConnectionBehavior {
    #[tracing::instrument(skip(self, _ctx))]
    fn connect(&mut self, local_id: NodeId, local_info: &NodeInfo, _ctx: &mut <Node as Actor>::Context) -> Result<(), NodeError> {
        Ok(())
    }

    #[tracing::instrument(skip(self, _ctx))]
    fn disconnect(&mut self, _ctx: &mut <Node as Actor>::Context) -> Result<(), NodeError> {
        Ok(())
    }
}


trait ProximityBehavior :
ChangeClusterBehavior +
ConnectionBehavior +
Debug + Display
{ }
