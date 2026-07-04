import os.path
import glob
from waflib import Task, TaskGen
from waflib.Tools.ccroot import stlink_task

top = '.'
out = 'build'

RUST_TARGETS = {
    'emery': 'thumbv7m-single-core',
}


class cargo_staticlib(stlink_task):
    always_run = True

    def run(self):
        env = dict(os.environ)
        env['PEBBLE_INCLUDE_DIRS'] = self.cargo_include_dirs
        env['PEBBLE_CFLAGS'] = self.cargo_cflags
        env['RUSTFLAGS'] = self.cargo_rustflags
        return self.exec_command(self.cargo_cmd, cwd=self.cargo_cwd, env=env)

    def keyword(self):
        return 'Compiling (cargo)'


@TaskGen.feature('cargo_staticlib')
def process_cargo_staticlib(tg):
    tsk = tg.create_task('cargo_staticlib')
    tsk.inputs = list(tg.cargo_header_deps)
    tsk.outputs = [tg.cargo_libnode]
    tsk.cargo_cmd = tg.cargo_cmd
    tsk.cargo_cwd = tg.cargo_cwd
    tsk.cargo_include_dirs = tg.cargo_include_dirs
    tsk.cargo_cflags = tg.cargo_cflags
    tsk.cargo_rustflags = tg.cargo_rustflags
    tg.link_task = tsk
    tg.target = tg.cargo_libname


def _declare_rust_staticlib(ctx, platform, rust_target, libname='bordstein_example'):
    rust_dir = ctx.path.find_node('.')
    target_dir = ctx.bldnode.make_node('target/{}'.format(platform))
    libnode = target_dir.make_node(
        '{}/release/examples/lib{}.a'.format(rust_target, libname))

    include_dirs = [
        os.path.join(ctx.env.PEBBLE_SDK_PLATFORM, 'include'),
        os.path.join(ctx.env.PEBBLE_SDK_COMMON, 'include'),
        ctx.bldnode.make_node(ctx.env.BUILD_DIR).abspath(),
        ctx.bldnode.make_node('include').abspath(),
    ]

    header_deps = [
        ctx.bldnode.make_node('include/message_keys.auto.h'),
        ctx.bldnode.make_node(ctx.env.BUILD_DIR).make_node(
            'src/resource_ids.auto.h'),
    ]

    name = 'rust_{}'.format(platform)
    ctx(
        features='cargo_staticlib',
        name=name,
        cargo_libname=libname,
        cargo_libnode=libnode,
        cargo_header_deps=header_deps,
        cargo_cwd=rust_dir.abspath(),
        cargo_include_dirs=':'.join(include_dirs),
        cargo_cflags=' '.join(ctx.env.CFLAGS),
        # Needed or the built program will be wonky, seems to cause it to not call functions correctly?
        cargo_rustflags='-C relocation-model=pic -C codegen-units=1 -C link-arg=--build-id=sha1 -C link-arg=--emit-relocs -C debuginfo=2 ',
        # The custom build target is needed so that rust assumes a single core and doesn't
        # generate barrier instructions (which work on the emulator, but fails on hardware)
        cargo_cmd=['cargo', 'build', '--release', '--example', 'bordstein-example',
                   '--target', rust_target + '.json',
                   '-Zjson-target-spec',
                   '--target-dir', target_dir.abspath()],
    )
    return name


def options(ctx):
    ctx.load('pebble_sdk')


def configure(ctx):
    ctx.load('pebble_sdk')


def build(ctx):
    ctx.load('pebble_sdk')

    build_worker = os.path.exists('worker_src')
    binaries = []

    cached_env = ctx.env
    for platform in ctx.env.TARGET_PLATFORMS:
        ctx.env = ctx.all_envs[platform]

        # silence some warnings
        ctx.env.LINKFLAGS += ["-z", "noexecstack"]
        ctx.env.LINKFLAGS += ["-Wl,--no-warn-rwx-segments"]

        ctx.set_group(ctx.env.PLATFORM_NAME)
        app_elf = '{}/pebble-app.elf'.format(ctx.env.BUILD_DIR)

        rust_kw = {}
        rust_target = RUST_TARGETS.get(platform)
        if rust_target:
            rust_kw['use'] = [_declare_rust_staticlib(ctx, platform, rust_target)]

        ctx.pbl_build(source=ctx.path.ant_glob('src/c/**/*.c'), target=app_elf,
                      bin_type='app', **rust_kw)

        if build_worker:
            worker_elf = '{}/pebble-worker.elf'.format(ctx.env.BUILD_DIR)
            binaries.append({'platform': platform, 'app_elf': app_elf, 'worker_elf': worker_elf})
            ctx.pbl_build(source=ctx.path.ant_glob('worker_src/c/**/*.c'),
                          target=worker_elf,
                          bin_type='worker', **rust_kw)
        else:
            binaries.append({'platform': platform, 'app_elf': app_elf})
    ctx.env = cached_env

    ctx.set_group('bundle')
    ctx.pbl_bundle(binaries=binaries,
                   js=ctx.path.ant_glob(['src/pkjs/**/*.js',
                                         'src/pkjs/**/*.json',
                                         'src/common/**/*.js']),
                   js_entry_file='src/pkjs/index.mjs')
