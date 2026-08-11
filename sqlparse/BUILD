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
load("@rules_rust//rust:defs.bzl", "rust_doc", "rust_library", "rust_test")

package(default_visibility = ["//visibility:public"])

genrule(
    name = "generate_parser",
    srcs = ["src/sqlstmt.lalrpop"],
    outs = [
        "src/sqlstmt.rs",
        # "sqlstmt.rs",
    ],
    cmd = """
       # TBD - cannot get lalrpop 0.22.2 executable, still shows 0.20.2
       # TBD - with --version option. May conflict with lalrpop_util version.
       
       /usr/bin/lalrpop $(location src/sqlstmt.lalrpop)
       
       echo "pwd:"
       pwd
       echo "ls -al:"
       ls -al
       echo "ls -al sqlparse:"
       ls -al sqlparse
       echo "ls -al sqlparse/src:"
       ls -al sqlparse/src
       cp sqlparse/src/sqlstmt.rs $(RULEDIR)/src/sqlstmt.rs
    """,
    #tools = ["@lalrpop//:lalrpop"],
)

rust_doc(
    name = "doc",
    crate = ":lib",
)

rust_library(
    name = "lib",
    srcs = [
        "src/ast.rs",
        "src/lexer.rs",
        "src/lib.rs",
        "src/sqlstmt.rs",
        ":generate_parser",  # Ensure parser is generated before building the library
    ],
    aliases = aliases(),
    crate_name = "sqlparser",
    deps = all_crate_deps(
        normal = True,
    ),
)

rust_test(
    name = "tests",
    srcs = [
        "tests/tests.rs",
    ],
    aliases = aliases(
        normal_dev = True,
        proc_macro_dev = True,
    ),
    proc_macro_deps = all_crate_deps(
        proc_macro_dev = True,
    ),
    rustc_env = {
        "RUST_BACKTRACE": "1",
    },
    deps = [
        ":lib",
    ] + all_crate_deps(
        normal_dev = True,
    ),
)

rust_test(
    name = "jsonpath",
    srcs = [
        "tests/jsonpath.rs",
    ],
    aliases = aliases(
        normal_dev = True,
        proc_macro_dev = True,
    ),
    proc_macro_deps = all_crate_deps(
        proc_macro_dev = True,
    ),
    rustc_env = {
        "RUST_BACKTRACE": "1",
    },
    deps = [
        ":lib",
    ] + all_crate_deps(
        normal_dev = True,
    ),
)
