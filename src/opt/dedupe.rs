//! # Port Map Deduplication Optimization Pass
//!
//! `portmap_dedupe` is an optimization pass that simplifies the port mappings
//! of component instances.  It eliminates duplicate entries in the component
//! instances' port mappings by introducing intermediate signals.  The VHDL
//! synthesizer depends on this pass.
