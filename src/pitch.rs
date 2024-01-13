use bevy::ecs::component::Component;
use plotters::{
    prelude::{BitMapBackend, ChartBuilder, IntoDrawingArea, PathElement},
    series::LineSeries,
    style::IntoFont,
};

#[derive(Clone, Component, Debug)]
pub enum Pitch {
    Sine { frequency: f32, volume: f32 },
}

impl Pitch {
    pub fn advance(&self, phase_accumulator: &mut PhaseAccumulator, sample_rate: f32) {
        match self {
            Pitch::Sine { frequency, volume } => {
                phase_accumulator.phase += (*frequency / sample_rate) * 2.0 * std::f32::consts::PI;
                if phase_accumulator.phase >= 2.0 * std::f32::consts::PI {
                    phase_accumulator.phase -= 2.0 * std::f32::consts::PI;
                }
            }
        }
    }

    pub fn wave(&self, phase_accumulator: &mut PhaseAccumulator, sample_rate: f32) -> f32 {
        match self {
            Pitch::Sine { frequency, volume } => (phase_accumulator.phase).sin() * volume,
        }
    }
}

#[derive(Debug)]
pub struct PhaseAccumulator {
    pub phase: f32,
}

pub fn plot_pitch(pitch: &Pitch, sample_rate: f32) -> Result<(), Box<dyn std::error::Error>> {
    let mut data: Vec<(f32, f32)> = vec![];
    let mut phase_accumulator = PhaseAccumulator { phase: 0.0 };

    for i in 1..100 {
        let wave_value = pitch.wave(&mut phase_accumulator, sample_rate);
        println!(
            "i: {}, phase: {}, wave: {}",
            i, phase_accumulator.phase, wave_value
        );
        data.push((i as f32, wave_value));
        pitch.advance(&mut phase_accumulator, sample_rate);
    }

    let root = BitMapBackend::new("output.png", (640, 480)).into_drawing_area();
    root.fill(&plotters::style::WHITE)?;
    let mut chart = ChartBuilder::on(&root)
        .caption("y=x^2", ("sans-serif", 50).into_font())
        .margin(5)
        .x_label_area_size(30)
        .y_label_area_size(30)
        .build_cartesian_2d(0f32..1000f32, -1f32..1f32)?;

    chart.configure_mesh().draw()?;

    chart
        .draw_series(LineSeries::new(data, &plotters::style::RED))?
        .label("Pitch")
        .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], &plotters::style::RED));

    chart
        .configure_series_labels()
        .background_style(&plotters::style::Color::mix(&plotters::style::WHITE, 0.8))
        .border_style(&plotters::style::BLACK)
        .draw()?;

    root.present()?;

    Ok(())
}
