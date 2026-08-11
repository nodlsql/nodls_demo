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
load("@rules_rust//rust:defs.bzl", "rust_doc", "rust_library")

package(default_visibility = ["//visibility:public"])

genrule(
    name = "geninsts",
    srcs = ["src/sqlinsts.proto"],
    outs = ["src/lib.rs"],
    cmd = """
       /usr/local/bin/protoc --proto_path=sqlinsts/src --prost_out=sqlinsts/src $(location src/sqlinsts.proto)
       /usr/bin/mv sqlinsts/src/sqlinsts.rs $(RULEDIR)/src/lib.rs
    """,
)

rust_doc(
    name = "doc",
    crate = ":lib",
)

rust_library(
    name = "lib",
    srcs = [
        "src/lib.rs",
    ],
    aliases = aliases(),
    crate_name = "sqlinsts",
    visibility = ["//visibility:public"],
    deps = [
    ] + all_crate_deps(
        normal = True,
    ),
)
