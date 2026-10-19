use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use clap::{Args, Parser, Subcommand, ValueEnum};
use displacement_gen::classic;
use displacement_gen::dots;
use displacement_gen::map::{Map, Palette};
use displacement_gen::panels::{self, Sprite};
use displacement_gen::velvet;
use displacement_gen::wyre;

#[derive(Parser)]
#[command(
    name = "displacement-cli",
    version,
    about = "Procedural sci-fi displacement, normal, and color maps",
    long_about = "Builds grayscale height maps for sci-fi surfaces, plus matching normal and color maps.\n\nModes follow the JSplacement workflow:\n  classic  plates, grilles, bars, and seams\n  panels   greeble stamps (the JSplacement 2 look)\n  velvet   mirrored recursive blocks\n  wyre     circuit wire traces\n  dots     sparse shapes and dot patterns\n\nA seed rebuilds the same map. Brighter height pixels are raised.",
    arg_required_else_help = true
)]
struct Cli {
    #[command(subcommand)]
    command: Command,

    /// Map width in pixels
    #[arg(long, global = true, default_value_t = 1024)]
    width: u32,

    /// Map height in pixels
    #[arg(long, global = true, default_value_t = 1024)]
    height: u32,

    /// Deterministic seed. Omit to pick one from the clock.
    #[arg(long, global = true)]
    seed: Option<u64>,

    /// Output directory
    #[arg(long, global = true, default_value = "out")]
    out: PathBuf,

    /// Flip height before normals and color are written
    #[arg(long, global = true)]
    invert: bool,

    /// Skip the normal map
    #[arg(long, global = true)]
    no_normal: bool,

    /// Skip the color map
    #[arg(long, global = true)]
    no_color: bool,

    /// Slope strength of the normal map
    #[arg(long, global = true, default_value_t = 4.0)]
    normal_strength: f32,

    /// Colorizer palette
    #[arg(long, global = true, value_enum, default_value_t = PaletteArg::Metal)]
    palette: PaletteArg,
}

#[derive(Subcommand)]
enum Command {
    /// Stacked plates, grilles, bars, and panel seams
    Classic(ClassicArgs),
    /// Greeble stamps in the JSplacement 2 style
    Panels(PanelsArgs),
    /// Mirrored recursive blocks
    Velvet(VelvetArgs),
    /// Angular circuit traces
    Wyre(WyreArgs),
    /// Sparse dots, shapes, and patterns
    Dots(DotsArgs),
    /// Generate every mode with its default settings
    All,
}

#[derive(Args)]
struct ClassicArgs {
    #[arg(long, default_value_t = classic::DEFAULT_ITERATIONS)]
    iterations: u32,
    #[arg(long, default_value_t = classic::DEFAULT_BACKGROUND)]
    background: u8,
    #[arg(long, default_value_t = classic::DEFAULT_MIN_BRIGHTNESS)]
    min_brightness: u8,
    #[arg(long, default_value_t = classic::DEFAULT_MAX_BRIGHTNESS)]
    max_brightness: u8,
    /// Largest shape as a percent of the short side
    #[arg(long, default_value_t = classic::DEFAULT_SCALE)]
    scale: f32,
    /// Opacity of the soft rectangle layer
    #[arg(long, default_value_t = classic::DEFAULT_GHOST_ALPHA)]
    ghost_alpha: f32,
    /// Percent chance a soft rectangle is a frame instead of a fill
    #[arg(long, default_value_t = classic::DEFAULT_GHOST_HOLE)]
    ghost_hole: f32,
    /// Gap between grille cells or bars, as a percent of their size
    #[arg(long, default_value_t = classic::DEFAULT_SPACING)]
    spacing: f32,
    #[arg(long, default_value_t = classic::DEFAULT_MAX_AMOUNT)]
    max_amount: u32,
    #[arg(long, default_value_t = classic::DEFAULT_LINE_WIDTH)]
    line_width: u32,
    #[arg(long)]
    no_squares: bool,
    #[arg(long)]
    no_ghosts: bool,
    #[arg(long)]
    no_grids: bool,
    #[arg(long)]
    no_vbars: bool,
    #[arg(long)]
    no_hbars: bool,
    #[arg(long)]
    no_crosses: bool,
}

#[derive(Args)]
struct PanelsArgs {
    #[arg(long, value_enum, default_value_t = PackArg::Crap)]
    pack: PackArg,
    #[arg(long, default_value_t = panels::DEFAULT_BACKGROUND)]
    background: u8,
    #[arg(long, default_value_t = panels::DEFAULT_BACKGROUND_ITERATIONS)]
    background_iterations: u32,
    #[arg(long, default_value_t = panels::DEFAULT_ITERATIONS)]
    iterations: u32,
    /// Largest stamp as a percent of the short side, before pack scaling
    #[arg(long, default_value_t = panels::DEFAULT_SCALE)]
    scale: f32,
    #[arg(long)]
    no_seamless: bool,
    #[arg(long)]
    no_rotate: bool,
    #[arg(long)]
    no_flip: bool,
    #[arg(long)]
    no_blend: bool,
    /// PNG or JPEG height stamps. When set, these replace the procedural pack.
    #[arg(long)]
    sprites: Option<PathBuf>,
}

#[derive(Args)]
struct VelvetArgs {
    #[arg(long, default_value_t = velvet::DEFAULT_ITERATIONS)]
    iterations: u32,
    #[arg(long, default_value_t = velvet::DEFAULT_SUB_ITERATIONS)]
    sub_iterations: u32,
    #[arg(long, default_value_t = velvet::DEFAULT_SPLITTER_ITERATIONS)]
    splitter_iterations: u32,
    #[arg(long, default_value_t = velvet::DEFAULT_BACKGROUND)]
    background: u8,
    /// xy mirrors both ways, x mirrors top-bottom, y mirrors left-right
    #[arg(long, value_enum, default_value_t = SymmetryArg::Xy)]
    symmetry: SymmetryArg,
    #[arg(long)]
    no_splitter: bool,
    #[arg(long)]
    no_splitter_blend: bool,
}

#[derive(Args)]
struct WyreArgs {
    #[arg(long, default_value_t = wyre::DEFAULT_LAYERS)]
    layers: u32,
    #[arg(long, default_value_t = wyre::DEFAULT_WIRES)]
    wires: u32,
    #[arg(long, default_value_t = wyre::DEFAULT_MAX_WIDTH)]
    max_width: u32,
    #[arg(long, default_value_t = wyre::DEFAULT_SPREAD)]
    spread: u32,
    #[arg(long, default_value_t = wyre::DEFAULT_MIN_STEP)]
    min_step: u32,
    #[arg(long, default_value_t = wyre::DEFAULT_MAX_STEP)]
    max_step: u32,
    #[arg(long, value_enum, default_value_t = RampArg::Linear)]
    color: RampArg,
    #[arg(long, value_enum, default_value_t = RampArg::Random)]
    width_mode: RampArg,
    #[arg(long, value_enum, default_value_t = DirArg::Any)]
    directions: DirArg,
}

#[derive(Args)]
struct DotsArgs {
    #[arg(long, default_value_t = dots::DEFAULT_SCALE)]
    scale: u32,
    #[arg(long, default_value_t = dots::DEFAULT_MIN_BRIGHTNESS)]
    min_brightness: u8,
    /// Percent chance a grid cell is drawn
    #[arg(long, default_value_t = dots::DEFAULT_CHANCE)]
    chance: f32,
    #[arg(long, default_value_t = dots::DEFAULT_MAX_LENGTH)]
    max_length: u32,
    #[arg(
        long,
        value_enum,
        value_delimiter = ',',
        default_values = ["square", "box", "circle", "ring", "diamond", "gem"]
    )]
    shapes: Vec<ShapeArg>,
    #[arg(
        long,
        value_enum,
        value_delimiter = ',',
        default_values = ["row", "col", "rake", "tee", "tri", "diag", "grid"]
    )]
    patterns: Vec<PatternArg>,
    #[arg(long)]
    break_code: bool,
}

#[derive(Clone, Copy, ValueEnum)]
enum PaletteArg {
    Metal,
    Cyan,
    Copper,
    Ember,
}

#[derive(Clone, Copy, ValueEnum)]
enum PackArg {
    Classic,
    Crap,
    #[value(name = "bigdata", alias = "big-data")]
    BigData,
    Aggro,
    Custom,
}

#[derive(Clone, Copy, ValueEnum)]
enum SymmetryArg {
    Xy,
    X,
    Y,
}

#[derive(Clone, Copy, ValueEnum)]
enum RampArg {
    Linear,
    #[value(name = "inverted-linear")]
    InvertedLinear,
    Curve,
    #[value(name = "inverted-curve")]
    InvertedCurve,
    Random,
}

#[derive(Clone, Copy, ValueEnum)]
enum DirArg {
    Any,
    Cardinal,
    Diagonal,
    Conical,
}

#[derive(Clone, Copy, ValueEnum)]
enum ShapeArg {
    Square,
    Box,
    Circle,
    Ring,
    Diamond,
    Gem,
}

#[derive(Clone, Copy, ValueEnum)]
enum PatternArg {
    Row,
    Col,
    Rake,
    Tee,
    Tri,
    Diag,
    Grid,
}

impl From<PaletteArg> for Palette {
    fn from(value: PaletteArg) -> Self {
        match value {
            PaletteArg::Metal => Palette::Metal,
            PaletteArg::Cyan => Palette::Cyan,
            PaletteArg::Copper => Palette::Copper,
            PaletteArg::Ember => Palette::Ember,
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    check_size(cli.width, cli.height)?;
    let seed = cli.seed.unwrap_or_else(clock_seed);
    println!("seed {seed}  {}x{}", cli.width, cli.height);

    match &cli.command {
        Command::Classic(args) => {
            let map = classic::generate(&classic_params(args), cli.width, cli.height, seed);
            emit("classic", map, &cli, seed, false)?;
        }
        Command::Panels(args) => {
            let params = panels_params(args)?;
            let wrap = params.seamless;
            let map = panels::generate(&params, cli.width, cli.height, seed);
            emit("panels", map, &cli, seed, wrap)?;
        }
        Command::Velvet(args) => {
            let map = velvet::generate(&velvet_params(args), cli.width, cli.height, seed);
            emit("velvet", map, &cli, seed, false)?;
        }
        Command::Wyre(args) => {
            let map = wyre::generate(&wyre_params(args), cli.width, cli.height, seed);
            emit("wyre", map, &cli, seed, false)?;
        }
        Command::Dots(args) => {
            let map = dots::generate(&dots_params(args), cli.width, cli.height, seed);
            emit("dots", map, &cli, seed, false)?;
        }
        Command::All => {
            emit(
                "classic",
                classic::generate(&classic::Params::default(), cli.width, cli.height, seed),
                &cli,
                seed,
                false,
            )?;
            let panels = panels::Params::default();
            let wrap = panels.seamless;
            emit(
                "panels",
                panels::generate(&panels, cli.width, cli.height, seed),
                &cli,
                seed,
                wrap,
            )?;
            emit(
                "velvet",
                velvet::generate(&velvet::Params::default(), cli.width, cli.height, seed),
                &cli,
                seed,
                false,
            )?;
            emit(
                "wyre",
                wyre::generate(&wyre::Params::default(), cli.width, cli.height, seed),
                &cli,
                seed,
                false,
            )?;
            emit(
                "dots",
                dots::generate(&dots::Params::default(), cli.width, cli.height, seed),
                &cli,
                seed,
                false,
            )?;
        }
    }
    Ok(())
}

fn emit(
    name: &str,
    mut map: Map,
    cli: &Cli,
    seed: u64,
    wrap_normals: bool,
) -> Result<(), Box<dyn Error>> {
    if cli.invert {
        map.invert();
    }
    fs::create_dir_all(&cli.out)?;
    let stem = format!("{name}_{seed}");
    let height_path = cli.out.join(format!("{stem}_height.png"));
    map.save_height(&height_path)?;
    println!("wrote {}", height_path.display());
    if !cli.no_normal {
        let path = cli.out.join(format!("{stem}_normal.png"));
        map.save_normal(&path, cli.normal_strength, wrap_normals)?;
        println!("wrote {}", path.display());
    }
    if !cli.no_color {
        let path = cli.out.join(format!("{stem}_color.png"));
        map.save_color(&path, cli.palette.into())?;
        println!("wrote {}", path.display());
    }
    Ok(())
}

fn classic_params(args: &ClassicArgs) -> classic::Params {
    classic::Params {
        iterations: args.iterations,
        background: args.background,
        min_brightness: args.min_brightness,
        max_brightness: args.max_brightness,
        scale: args.scale,
        ghost_alpha: args.ghost_alpha,
        ghost_hole: args.ghost_hole,
        spacing: args.spacing,
        max_amount: args.max_amount,
        line_width: args.line_width,
        squares: !args.no_squares,
        ghosts: !args.no_ghosts,
        grids: !args.no_grids,
        vbars: !args.no_vbars,
        hbars: !args.no_hbars,
        crosses: !args.no_crosses,
    }
}

fn panels_params(args: &PanelsArgs) -> Result<panels::Params, Box<dyn Error>> {
    let custom = if let Some(dir) = &args.sprites {
        load_sprites(dir)?
    } else if matches!(args.pack, PackArg::Custom) {
        return Err(
            "the custom pack needs --sprites <directory> of PNG or JPEG height stamps".into(),
        );
    } else {
        Vec::new()
    };
    let pack = if args.sprites.is_some() {
        panels::Pack::Custom
    } else {
        match args.pack {
            PackArg::Classic => panels::Pack::Classic,
            PackArg::Crap => panels::Pack::Crap,
            PackArg::BigData => panels::Pack::BigData,
            PackArg::Aggro => panels::Pack::Aggro,
            PackArg::Custom => panels::Pack::Custom,
        }
    };
    Ok(panels::Params {
        pack,
        background: args.background,
        background_iterations: args.background_iterations,
        iterations: args.iterations,
        scale: args.scale,
        seamless: !args.no_seamless,
        rotate: !args.no_rotate,
        flip: !args.no_flip,
        blend: !args.no_blend,
        custom,
    })
}

fn velvet_params(args: &VelvetArgs) -> velvet::Params {
    velvet::Params {
        iterations: args.iterations,
        sub_iterations: args.sub_iterations,
        splitter_iterations: args.splitter_iterations,
        background: args.background,
        symmetry: match args.symmetry {
            SymmetryArg::Xy => velvet::Symmetry::Both,
            SymmetryArg::X => velvet::Symmetry::X,
            SymmetryArg::Y => velvet::Symmetry::Y,
        },
        use_splitter: !args.no_splitter,
        splitter_blend: !args.no_splitter_blend,
    }
}

fn wyre_params(args: &WyreArgs) -> wyre::Params {
    wyre::Params {
        layers: args.layers,
        wires_per_layer: args.wires,
        max_width: args.max_width,
        origin_spread: args.spread,
        min_step: args.min_step,
        max_step: args.max_step,
        color: ramp(args.color),
        width: ramp(args.width_mode),
        directions: match args.directions {
            DirArg::Any => wyre::Directions::Any,
            DirArg::Cardinal => wyre::Directions::Cardinal,
            DirArg::Diagonal => wyre::Directions::Diagonal,
            DirArg::Conical => wyre::Directions::Conical,
        },
    }
}

fn dots_params(args: &DotsArgs) -> dots::Params {
    dots::Params {
        scale: args.scale,
        min_brightness: args.min_brightness,
        chance: args.chance,
        max_length: args.max_length,
        shapes: args.shapes.iter().copied().map(shape).collect(),
        patterns: args.patterns.iter().copied().map(pattern).collect(),
        break_code: args.break_code,
    }
}

fn ramp(value: RampArg) -> wyre::Ramp {
    match value {
        RampArg::Linear => wyre::Ramp::Linear,
        RampArg::InvertedLinear => wyre::Ramp::InvertedLinear,
        RampArg::Curve => wyre::Ramp::Curve,
        RampArg::InvertedCurve => wyre::Ramp::InvertedCurve,
        RampArg::Random => wyre::Ramp::Random,
    }
}

fn shape(value: ShapeArg) -> dots::Shape {
    match value {
        ShapeArg::Square => dots::Shape::Square,
        ShapeArg::Box => dots::Shape::Box,
        ShapeArg::Circle => dots::Shape::Circle,
        ShapeArg::Ring => dots::Shape::Ring,
        ShapeArg::Diamond => dots::Shape::Diamond,
        ShapeArg::Gem => dots::Shape::Gem,
    }
}

fn pattern(value: PatternArg) -> dots::Pattern {
    match value {
        PatternArg::Row => dots::Pattern::Row,
        PatternArg::Col => dots::Pattern::Col,
        PatternArg::Rake => dots::Pattern::Rake,
        PatternArg::Tee => dots::Pattern::Tee,
        PatternArg::Tri => dots::Pattern::Tri,
        PatternArg::Diag => dots::Pattern::Diag,
        PatternArg::Grid => dots::Pattern::Grid,
    }
}

fn load_sprites(dir: &Path) -> Result<Vec<Sprite>, Box<dyn Error>> {
    if !dir.is_dir() {
        return Err(format!("sprite directory not found: {}", dir.display()).into());
    }
    let mut paths = Vec::new();
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        let ext = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase());
        if matches!(ext.as_deref(), Some("png") | Some("jpg") | Some("jpeg")) {
            paths.push(path);
        }
    }
    paths.sort();
    if paths.is_empty() {
        return Err(format!("no PNG or JPEG files in {}", dir.display()).into());
    }
    let mut sprites = Vec::with_capacity(paths.len());
    for path in paths {
        let image = image::open(&path)?.to_rgba8();
        sprites.push(Sprite::from_rgba(&image));
    }
    Ok(sprites)
}

fn check_size(width: u32, height: u32) -> Result<(), Box<dyn Error>> {
    if !(1..=8192).contains(&width) || !(1..=8192).contains(&height) {
        return Err(format!("width and height must be 1..=8192, got {width}x{height}").into());
    }
    Ok(())
}

fn clock_seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(1)
}
