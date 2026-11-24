//! Initial native-window placement within a monitor's usable desktop bounds.
use tauri::{Monitor, PhysicalPosition, PhysicalSize, WebviewWindow};

/// Fits a hidden window before showing it. Existing windows keep user placement.
pub fn show_fitted(
    window: &WebviewWindow,
    minimum: (f64, f64),
    preferred_monitor: Option<Monitor>,
) -> tauri::Result<()> {
    let monitor = match preferred_monitor {
        Some(monitor) => Some(monitor),
        None => window.current_monitor()?.or(window.primary_monitor()?),
    };
    if let Some(monitor) = monitor {
        let work = monitor.work_area();
        let scale = monitor.scale_factor();
        let current_scale = window.scale_factor()?;
        // Preserve logical size when Welcome and the new window start on
        // monitors with different pixel densities.
        let inner = window.inner_size()?.to_logical::<f64>(current_scale).to_physical::<u32>(scale);
        let outer = window.outer_size()?.to_logical::<f64>(current_scale).to_physical::<u32>(scale);
        let (size, position) =
            fitted_bounds(work.position, work.size, inner, outer, (16.0 * scale).round() as u32);
        // A minimum taller than a laptop's work area must not prevent fitting.
        window.set_min_size(Some(
            PhysicalSize::new(
                ((minimum.0 * scale).round() as u32).min(size.width),
                ((minimum.1 * scale).round() as u32).min(size.height),
            )
            .to_logical::<f64>(scale),
        ))?;
        window.set_size(size.to_logical::<f64>(scale))?;
        window.set_position(position)?;
    }
    window.show()?;
    window.set_focus()
}

/// Preserves the requested size when possible, accounting for native decorations.
fn fitted_bounds(
    origin: PhysicalPosition<i32>,
    work: PhysicalSize<u32>,
    inner: PhysicalSize<u32>,
    outer: PhysicalSize<u32>,
    margin: u32,
) -> (PhysicalSize<u32>, PhysicalPosition<i32>) {
    let border_width = outer.width.saturating_sub(inner.width);
    let border_height = outer.height.saturating_sub(inner.height);
    let width = inner.width.min(work.width.saturating_sub(2 * margin + border_width).max(1));
    let height = inner.height.min(work.height.saturating_sub(2 * margin + border_height).max(1));
    let position = PhysicalPosition::new(
        origin.x + (work.width.saturating_sub(width + border_width) / 2) as i32,
        origin.y + (work.height.saturating_sub(height + border_height) / 2) as i32,
    );
    (PhysicalSize::new(width, height), position)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fits_oversized_window_between_menu_bar_and_dock() {
        let (size, position) = fitted_bounds(
            PhysicalPosition::new(0, 25),
            PhysicalSize::new(1440, 825),
            PhysicalSize::new(1280, 1040),
            PhysicalSize::new(1280, 1068),
            16,
        );
        assert_eq!(size, PhysicalSize::new(1280, 765));
        assert_eq!(position, PhysicalPosition::new(80, 41));
        assert_eq!(position.y + size.height as i32 + 28, 834);
    }

    #[test]
    fn retains_size_and_centers_on_a_retina_monitor_with_negative_origin() {
        let (size, position) = fitted_bounds(
            PhysicalPosition::new(-2880, 50),
            PhysicalSize::new(2880, 1700),
            PhysicalSize::new(2120, 1400),
            PhysicalSize::new(2120, 1456),
            32,
        );
        assert_eq!(size, PhysicalSize::new(2120, 1400));
        assert_eq!(position, PhysicalPosition::new(-2500, 172));
    }

    #[test]
    fn reduces_both_dimensions_on_a_small_work_area() {
        let (size, position) = fitted_bounds(
            PhysicalPosition::new(1200, 40),
            PhysicalSize::new(800, 600),
            PhysicalSize::new(1060, 820),
            PhysicalSize::new(1070, 850),
            16,
        );
        assert_eq!(size, PhysicalSize::new(758, 538));
        assert_eq!(position, PhysicalPosition::new(1216, 56));
    }
}
