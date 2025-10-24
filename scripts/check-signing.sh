#!/bin/bash

# Check macOS Code Signing Setup
# This script verifies your code signing configuration

set -e

echo "🔍 Checking macOS Code Signing Setup..."
echo ""

# Check for certificates
echo "📜 Available Code Signing Certificates:"
security find-identity -v -p codesigning | grep -v "0 valid identities found" || {
    echo "❌ No code signing certificates found!"
    echo "   You need a 'Developer ID Application' certificate."
    echo "   See SIGNING.md for instructions."
    exit 1
}
echo ""

# Check for Developer ID certificate
if security find-identity -v -p codesigning | grep -q "Developer ID Application"; then
    echo "✅ Developer ID Application certificate found"
    CERT_NAME=$(security find-identity -v -p codesigning | grep "Developer ID Application" | head -1 | sed 's/.*"\(.*\)"/\1/')
    echo "   Certificate: $CERT_NAME"
else
    echo "⚠️  No 'Developer ID Application' certificate found"
    echo "   You have development certificates, but need a distribution certificate."
    echo "   See SIGNING.md Step 1 for instructions."
fi
echo ""

# Check Tauri config
echo "⚙️  Checking Tauri Configuration:"
if [ -f "src-ui/src-tauri/tauri.conf.json" ]; then
    SIGNING_IDENTITY=$(grep -A 6 '"macOS"' src-ui/src-tauri/tauri.conf.json | grep 'signingIdentity' | cut -d'"' -f4)
    if [ "$SIGNING_IDENTITY" = "null" ] || [ -z "$SIGNING_IDENTITY" ]; then
        echo "⚠️  signingIdentity is not configured in tauri.conf.json"
        echo "   Update 'bundle.macOS.signingIdentity' with your certificate name"
    else
        echo "✅ signingIdentity configured: $SIGNING_IDENTITY"
    fi
else
    echo "❌ tauri.conf.json not found"
fi
echo ""

# Check entitlements
echo "📋 Checking Entitlements:"
if [ -f "src-ui/src-tauri/Entitlements.plist" ]; then
    echo "✅ Entitlements.plist exists"

    # Check for audio permissions
    if grep -q "com.apple.security.device.audio-input" src-ui/src-tauri/Entitlements.plist; then
        echo "✅ Audio input permission configured"
    else
        echo "⚠️  Audio input permission missing"
    fi

    # Check for network permissions
    if grep -q "com.apple.security.network.client" src-ui/src-tauri/Entitlements.plist; then
        echo "✅ Network client permission configured"
    else
        echo "⚠️  Network client permission missing"
    fi
else
    echo "❌ Entitlements.plist not found"
fi
echo ""

# Check for notarization credentials
echo "🔐 Checking Notarization Setup:"
if [ -n "$APPLE_ID" ]; then
    echo "✅ APPLE_ID environment variable set: $APPLE_ID"
else
    echo "⚠️  APPLE_ID environment variable not set"
fi

if [ -n "$APPLE_TEAM_ID" ]; then
    echo "✅ APPLE_TEAM_ID environment variable set: $APPLE_TEAM_ID"
else
    echo "⚠️  APPLE_TEAM_ID environment variable not set"
fi

if [ -n "$APPLE_SIGNING_IDENTITY" ]; then
    echo "✅ APPLE_SIGNING_IDENTITY environment variable set"
else
    echo "⚠️  APPLE_SIGNING_IDENTITY environment variable not set"
fi
echo ""

# Check if notarytool credentials are stored
echo "🔑 Checking Stored Notarization Credentials:"
if xcrun notarytool history --keychain-profile "autoeq-notarization" 2>/dev/null | grep -q "Successfully received submission history"; then
    echo "✅ Notarization credentials stored in keychain"
else
    echo "⚠️  No notarization credentials found in keychain"
    echo "   Run: xcrun notarytool store-credentials (see SIGNING.md Step 4)"
fi
echo ""

# Summary
echo "📊 Summary:"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

HAS_DEV_ID=false
if security find-identity -v -p codesigning | grep -q "Developer ID Application"; then
    HAS_DEV_ID=true
fi

if [ "$HAS_DEV_ID" = true ] && [ -n "$APPLE_ID" ] && [ -n "$APPLE_TEAM_ID" ]; then
    echo "✅ Ready for signing and notarization!"
    echo ""
    echo "Next steps:"
    echo "  1. Build: cd src-ui && npm run tauri build"
    echo "  2. The app will be automatically signed and submitted for notarization"
    echo "  3. Find the signed DMG in: src-ui/src-tauri/target/release/bundle/dmg/"
elif [ "$HAS_DEV_ID" = true ]; then
    echo "⚠️  Certificate ready, but notarization not configured"
    echo ""
    echo "Next steps:"
    echo "  1. Set up notarization (see SIGNING.md Step 4)"
    echo "  2. Build: npm run tauri build"
else
    echo "❌ Not ready for distribution"
    echo ""
    echo "Next steps:"
    echo "  1. Get a Developer ID Application certificate (see SIGNING.md Step 1)"
    echo "  2. Configure notarization (see SIGNING.md Step 4)"
    echo "  3. Update tauri.conf.json with your certificate"
fi
echo ""
echo "For detailed instructions, see: SIGNING.md"
