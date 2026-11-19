//! The rules of the pack's Hostile Neural Networks port, free of the engine so they can be tested alone: the model
//! tiers and the data models' numbers, the config's power caps, the Simulation Chamber's cycle (inference and
//! training) and the Loot Fabricator's. Translated from Hostile Neural Networks 6.5.1
//! (Shadows-of-Fire/Hostile-Neural-Networks, MIT); see the bundle's README.md and LICENSE-hostilenetworks.
pub mod config;
pub mod fab;
pub mod model;
pub mod sim;
pub mod tier;
pub mod view;
