use bobbin_bits::U4;
use sdl2::{render::Canvas, video::Window, pixels::Color, event::Event, keyboard::Keycode, rect::Rect, audio::{AudioCallback, AudioSpecDesired, AudioDevice}};
use crate::chip8::CPU;

/// The default Windows graphics (and audio) layer, implemented using SDL2.
pub struct WindowsSDL2{
    cycles_per_frame: u8,
}

const PIXEL_SIZE: i32 = 20;

impl WindowsSDL2{
    /// Creates a new instance of the Windows graphics (and audio) layer.
    /// Defaults to 10 CPU cycles per frame, at 60fps.
    pub fn new() -> Self {WindowsSDL2 { cycles_per_frame: 10 }}

    /// Starts the interpreter. Responsible for handling inputs, driving the CPU and timers, and triggering audio.
    pub fn start_interpreter(&mut self, cpu: &mut CPU) -> Result<(), String> {
        // Initialise SDL alongside video and audio subsystems
        let sdl_context = sdl2::init()?;
        let video_subsystem = sdl_context.video()?;
        let audio_subsystem = sdl_context.audio()?;

        // Create window
        let window = video_subsystem
            .window("CHIP-8 Interpreter", 1280, 640)
            .position_centered()
            .opengl()
            .build()
            .map_err(|e| e.to_string())?;

        // Create canvas which is mapped onto window
        let mut canvas = window.into_canvas().present_vsync()
            .build().map_err(|e| e.to_string())?;

        canvas.set_draw_color(Color::RGB(0, 0, 0));
        canvas.clear();
        canvas.present();

        // Define the expected sound for the buzzer
        let desired_spec = AudioSpecDesired {
            freq: Some(44100),
            channels: Some(1),  // Mono audio
            samples: None       // Default sample size
        };

        // Initialise the audio source
        let mut device = audio_subsystem.open_playback(None, 
            &desired_spec, |spec| {
            SquareWave {
                phase_inc: 440.0 / spec.freq as f32,
                phase: 0.0,
                volume: 0.25
            }
        }).unwrap();

        // Get event handler and timer objects
        let mut event_pump = sdl_context.event_pump()?;
        let mut timer = sdl_context.timer()?;

        // Define expected frame timing (60fps)
        let frame_interval = 1_000 / 60;
        let mut last_frame_time = timer.ticks();
    
        'running: loop {
            // Handle key presses
            for event in event_pump.poll_iter() {
                match event {
                    Event::Quit { .. } => break 'running,
                    Event::KeyDown { keycode: Some(Keycode::Escape), .. } => break 'running,
                    // +/- adjust the game speed by changing the CPU cycles per frame
                    Event::KeyDown{ keycode: Some(Keycode::Minus), .. } => {
                        if self.cycles_per_frame > 0  { self.cycles_per_frame -= 1; } 
                    },
                    Event::KeyDown{ keycode: Some(Keycode::Plus), .. } => self.cycles_per_frame += 1,
                    Event::KeyDown{ keycode: Some(Keycode::Equals), .. } => self.cycles_per_frame += 1,
                    Event::KeyDown { keycode: Some(keycode), .. } => self.handle_key(cpu, keycode, true),
                    Event::KeyUp { keycode: Some(keycode), .. } => self.handle_key(cpu, keycode, false),
                    _ => { }
                }
            }
            
            // These statement will execute once per frame, or once roughly every 16.67ms, so we update the sound/delay timers
            cpu.tick();
            self.play_tone(cpu, &mut device);
            
            // At 60fps, the default ten instructions per frame equals 60 * 10 = 600 instructions per second
            for _instruction in 0..self.cycles_per_frame{
                cpu.step();
            }

            // Draw latest frame
            self.draw_frame(cpu, &mut canvas);

            // Calculate time since last frame
            let current_frame_time = timer.ticks();
            let delta = current_frame_time - last_frame_time;
            
            // Enforce frame timing: execution doesn't continue until 16.67ms has passed since last frame
            if delta < frame_interval {
                timer.delay(frame_interval - delta);
                continue;
            }
            last_frame_time = current_frame_time;
        }
        
        Ok(())
    }

    /// Handles key presses. Keys are mapped to the top-left of the keyboard, following standard convention.
    /// The `state` parameter determines whether the key is pressed / released.
    fn handle_key(&mut self, cpu: &mut CPU, keycode: Keycode, state: bool) {
        match keycode {
            Keycode::Num1 => cpu.update_key(U4::B0001, state),
            Keycode::Num2 => cpu.update_key(U4::B0010, state),
            Keycode::Num3 => cpu.update_key(U4::B0011, state),
            Keycode::Num4 => cpu.update_key(U4::B1100, state),
            Keycode::Q => cpu.update_key(U4::B0100, state),
            Keycode::W => cpu.update_key(U4::B0101, state),
            Keycode::E => cpu.update_key(U4::B0110, state),
            Keycode::R => cpu.update_key(U4::B1101, state),
            Keycode::A => cpu.update_key(U4::B0111, state),
            Keycode::S => cpu.update_key(U4::B1000, state),
            Keycode::D => cpu.update_key(U4::B1001, state),
            Keycode::F => cpu.update_key(U4::B1110, state),
            Keycode::Z => cpu.update_key(U4::B1010, state),
            Keycode::X => cpu.update_key(U4::B0000, state),
            Keycode::C => cpu.update_key(U4::B1011, state),
            Keycode::V => cpu.update_key(U4::B1111, state),
            _ => return
        };
    }

    /// Draws the next frame to the screen.
    fn draw_frame(&mut self, cpu: &CPU, canvas: &mut Canvas<Window>) {
        // Clear the screen.
        canvas.set_draw_color(Color::RGB(0,0, 0));
        canvas.clear();

        // For now, pixels are white.
        canvas.set_draw_color(Color::RGB(255,255, 255));
        
        // Iterate over the 2D array storing the screen state, and draw a pixel if the corresponding value is set to true.
        for x in 0..64{
            for y in 0..32{
                if cpu.screen[y][x] { 
                    let _ = canvas.fill_rect(Rect::new(
                        x as i32 * PIXEL_SIZE,
                        y as i32 * PIXEL_SIZE,
                        PIXEL_SIZE as u32,
                        PIXEL_SIZE as u32,
                    ));
                }
            }
        }
        canvas.present();
    }

    /// Turns on/off the buzzer based on the value of the sound timer.
    fn play_tone(&mut self, cpu: &CPU, audio: &mut AudioDevice<SquareWave>) {
        if cpu.st > 0 { audio.resume(); }
        else { audio.pause(); }
    }
}

/// A representation of a square wave.
struct SquareWave {
    phase_inc: f32,
    phase: f32,
    volume: f32
}

impl AudioCallback for SquareWave {
    type Channel = f32;

    fn callback(&mut self, out: &mut [f32]) {
        // Generates a square wave.
        for x in out.iter_mut() {
            *x = if self.phase <= 0.5 {
                self.volume
            } else {
                -self.volume
            };
            self.phase = (self.phase + self.phase_inc) % 1.0;
        }
    }
}