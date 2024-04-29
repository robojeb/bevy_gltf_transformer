# `bevy_gltf_transformer`

This crate provides a trait to implement a custom glTF asset loader that can 
perform transformations on the incoming glTF data. 
This is intended as an alternative to asset pipelines that require the glTF 
asset to be loaded, spawned, and then modified by run-time systems. 

The major advantage to load-time transformation is that it reduces potential 
issues with system ordering when gameplay systems may not be aware of asset
modification systems.