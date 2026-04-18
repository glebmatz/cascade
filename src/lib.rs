// Lints we deliberately allow project-wide.
#![allow(clippy::needless_range_loop)] // explicit indexing reads better in DSP code
#![allow(clippy::question_mark)] // some let...else cases are clearer than `?`
#![allow(clippy::while_let_loop)] // explicit `loop { ... break; }` is fine for codecs

pub mod achievements;
pub mod app;
pub mod audio;
pub mod beatmap;
pub mod cli;
pub mod config;
pub mod game;
pub mod input;
pub mod play_history;
pub mod score_store;
pub mod screens;
pub mod stats;
pub mod ui;
