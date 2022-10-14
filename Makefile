test:
	cargo test --all -- --nocapture

check:
	cargo check --all --all-targets --all-features

fmt:
	cargo fmt --all -- --check

clippy_rule = -D warnings \
	-D clippy::clone_on_ref_ptr \
	-D clippy::enum_glob_use \
	-A clippy::collapsible-else-if \
	-A clippy::upper_case_acronyms \
	-A clippy::unusual_byte_groupings \
	-A clippy::inconsistent_digit_grouping \
	-A clippy::large_digit_groups \
	-A clippy::suspicious_operation_groupings
clippy:
	cargo clippy --all -- $(clippy_rule)

ci: fmt check clippy test
	git diff --exit-code Cargo.lock

src/aot.x64.compiled.c: src/aot.x64.c .deps/luajit/src/host/minilua
	.deps/luajit/src/host/minilua .deps/luajit/dynasm/dynasm.lua -o $@ $<

src/aot.x64.win.compiled.c: src/aot.x64.c .deps/luajit/src/host/minilua
	.deps/luajit/src/host/minilua .deps/luajit/dynasm/dynasm.lua -D WIN -o $@ $<

.deps/luajit/src/host/minilua:
	rm -rf .deps/luajit && mkdir -p .deps && \
		git clone https://github.com/LuaJIT/LuaJIT .deps/luajit && \
		cd .deps/luajit && git checkout 0f8a340c8c71fb8f5b8ae7c3ae94bfe81af8f8e8 && \
		make
