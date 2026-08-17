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

rust_doc(
    name = "doc",
    crate = ":lib",
)

rust_library(
    name = "lib",
    srcs = [
        "src/lib.rs",
        "src/utils.rs",
    ],
    aliases = aliases(),
    crate_name = "sqlcontrols",
    deps = [
        "//demoexe:lib",
        "//indexbt:lib",
        "//indexsrch:lib",
        "//jbparse:lib",
        "//relpart:lib",
        "//sqlexet:lib",
        "//sqlinsts:lib",
        "//sqloptimize:lib",
        "//sqlparse:lib",
        "//sqlplan:lib",
    ] + all_crate_deps(
        normal = True,
    ),
)

# Execute all the test targets
[rust_test(
    name = f.replace(".rs", ""),
    srcs = [
        f,
    ],
    crate_root = f,
    deps = [
        ":lib",
        "//demoexe:lib",
        "//jbparse:lib",
        "//sqlexet:lib",
        "//sqlinsts:lib",
        "//sqloptimize:lib",
        "//sqlparse:lib",
        "//sqlplan:lib",
    ] + all_crate_deps(
        normal = True,
        proc_macro = True,
    ),
) for f in glob(["tests/*.rs"])]

rust_test(
    name = "alter",
    srcs = [
        "tests/alter.rs",
    ],
    crate_root = "tests/alter.rs",
    deps = [
        ":lib",
        "//demoexe:lib",
        "//jbparse:lib",
        "//sqlexet:lib",
        "//sqlinsts:lib",
        "//sqlparse:lib",
        "//sqlplan:lib",
    ] + all_crate_deps(
        normal = True,
        proc_macro = True,
    ),
)

rust_test(
    name = "delete",
    srcs = [
        "tests/delete.rs",
    ],
    crate_root = "tests/delete.rs",
    deps = [
        ":lib",
        "//demoexe:lib",
        "//jbparse:lib",
        "//sqlexet:lib",
        "//sqlinsts:lib",
        "//sqlparse:lib",
        "//sqlplan:lib",
    ] + all_crate_deps(
        normal = True,
        proc_macro = True,
    ),
)

rust_test(
    name = "dupidx",
    srcs = [
        "tests/dupidx.rs",
    ],
    crate_root = "tests/dupidx.rs",
    deps = [
        ":lib",
        "//demoexe:lib",
        "//jbparse:lib",
        "//sqlexet:lib",
        "//sqlinsts:lib",
        "//sqlparse:lib",
        "//sqlplan:lib",
    ] + all_crate_deps(
        normal = True,
        proc_macro = True,
    ),
)

rust_test(
    name = "isnull",
    srcs = [
        "tests/isnull.rs",
    ],
    crate_root = "tests/isnull.rs",
    deps = [
        ":lib",
        "//demoexe:lib",
        "//jbparse:lib",
        "//sqlexet:lib",
        "//sqlinsts:lib",
        "//sqloptimize:lib",
        "//sqlparse:lib",
        "//sqlplan:lib",
    ] + all_crate_deps(
        normal = True,
        proc_macro = True,
    ),
)

rust_test(
    name = "inregex",
    srcs = [
        "tests/inregex.rs",
    ],
    crate_root = "tests/inregex.rs",
    deps = [
        ":lib",
        "//demoexe:lib",
        "//jbparse:lib",
        "//sqlexet:lib",
        "//sqlinsts:lib",
        "//sqloptimize:lib",
        "//sqlparse:lib",
        "//sqlplan:lib",
    ] + all_crate_deps(
        normal = True,
        proc_macro = True,
    ),
)

# Execute jsonpath test target
rust_test(
    name = "jsonpath",
    srcs = [
        "tests/jsonpath.rs",
    ],
    crate_root = "tests/jsonpath.rs",
    deps = [
        ":lib",
        "//demoexe:lib",
        "//jbparse:lib",
        "//sqlexet:lib",
        "//sqlinsts:lib",
        "//sqloptimize:lib",
        "//sqlparse:lib",
        "//sqlplan:lib",
    ] + all_crate_deps(
        normal = True,
        proc_macro = True,
    ),
)

# Execute rels test target
rust_test(
    name = "rels",
    srcs = [
        "tests/rels.rs",
    ],
    crate_root = "tests/rels.rs",
    deps = [
        ":lib",
        "//demoexe:lib",
        "//jbparse:lib",
        "//sqlexet:lib",
        "//sqlinsts:lib",
        "//sqlparse:lib",
        "//sqlplan:lib",
    ] + all_crate_deps(
        normal = True,
        proc_macro = True,
    ),
)  # Execute select test target

# Execute select test target
rust_test(
    name = "select",
    srcs = [
        "tests/select.rs",
    ],
    crate_root = "tests/select.rs",
    deps = [
        ":lib",
        "//demoexe:lib",
        "//jbparse:lib",
        "//sqlexet:lib",
        "//sqlinsts:lib",
        "//sqlparse:lib",
        "//sqlplan:lib",
    ] + all_crate_deps(
        normal = True,
        proc_macro = True,
    ),
)  # Execute select test target

rust_test(
    name = "tests",
    srcs = [
        "tests/tests.rs",
    ],
    crate_root = "tests/tests.rs",
    deps = [
        ":lib",
        "//demoexe:lib",
        "//jbparse:lib",
        "//sqlexet:lib",
        "//sqlinsts:lib",
        "//sqloptimize:lib",
        "//sqlparse:lib",
        "//sqlplan:lib",
    ] + all_crate_deps(
        normal = True,
        proc_macro = True,
    ),
)

rust_test(
    name = "update",
    srcs = [
        "tests/update.rs",
    ],
    crate_root = "tests/update.rs",
    deps = [
        ":lib",
        "//demoexe:lib",
        "//jbparse:lib",
        "//sqlexet:lib",
        "//sqlinsts:lib",
        "//sqlparse:lib",
        "//sqlplan:lib",
    ] + all_crate_deps(
        normal = True,
        proc_macro = True,
    ),
)

rust_test(
    name = "yank",
    srcs = [
        "tests/yank.rs",
    ],
    crate_root = "tests/yank.rs",
    deps = [
        ":lib",
        "//demoexe:lib",
        "//jbparse:lib",
        "//sqlexet:lib",
        "//sqlinsts:lib",
        "//sqlparse:lib",
        "//sqlplan:lib",
    ] + all_crate_deps(
        normal = True,
        proc_macro = True,
    ),
)
