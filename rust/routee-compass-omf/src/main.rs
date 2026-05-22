use clap::Parser;
use routee_compass_omf::{app::OmfApp, collection::OvertureMapsCollectionError};

fn main() -> Result<(), OvertureMapsCollectionError> {
    env_logger::init();
    let args = OmfApp::parse();
    args.op.run()
}
