id := "st.lynx.plugins.opendeck-akp03.sdPlugin"

package: build-linux build-mac build-win collect zip

build-linux:
    cargo build --release --target x86_64-unknown-linux-gnu --target-dir target/plugin-linux

build-mac:
    cross build --release --target x86_64-apple-darwin --target aarch64-apple-darwin --target-dir target/plugin-mac

build-win:
    cargo build --release --target x86_64-pc-windows-gnu --target-dir target/plugin-win

collect:
    rm -r build
    mkdir -p build/{{id}}
    cp -r assets build/{{id}}
    cp manifest.json build/{{id}}
    cp target/plugin-linux/x86_64-unknown-linux-gnu/release/opendeck-akp03 build/{{id}}/opendeck-akp03-linux
    cp target/plugin-win/x86_64-pc-windows-gnu/release/opendeck-akp03.exe build/{{id}}/opendeck-akp03-win.exe
    lipo -create -output build/{{id}}/opendeck-akp03-mac target/plugin-mac/x86_64-apple-darwin/release/opendeck-akp03 target/plugin-mac/aarch64-apple-darwin/release/opendeck-akp03

[working-directory: "build"]
zip:
    zip -r opendeck-akp03.plugin.zip {{id}}/
