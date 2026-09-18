# Copyright 2026 No Despondency Labs.
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

load("@crates//:defs.bzl", "aliases", "all_crate_deps")
load("@rules_rust//cargo:defs.bzl", "cargo_build_script")
load("@rules_rust//rust:defs.bzl", "rust_doc", "rust_library")

package(default_visibility = ["//visibility:public"])

cargo_build_script(
    name = "build_script",
    srcs = ["build.rs"],
    data = ["src/indexkey.capnp"],
    edition = "2021",
    deps = all_crate_deps(
        build = True,
    ),
)

rust_doc(
    name = "doc",
    crate = ":lib",
    rustdoc_flags = ["--cfg=bazel_build"],
)

rust_library(
    name = "lib",
    srcs = [
        "src/lib.rs",
    ],
    aliases = aliases(),
    crate_name = "indexcapn",
    rustc_flags = ["--cfg=bazel_build"],
    crate_root = "src/lib.rs",
    visibility = ["//visibility:public"],
    deps = [
        ":build_script",
    ] + all_crate_deps(
        normal = True,
    ),
)
