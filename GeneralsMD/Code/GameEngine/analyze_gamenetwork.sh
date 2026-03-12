#!/bin/bash

echo "=========================================================="
echo "GameNetwork Usage Analysis - Finding actual usage patterns"
echo "=========================================================="

# First, let's find how GameNetwork is actually used via global objects
echo ""
echo "1. Searching for GameNetwork usage via global objects..."
echo ""

echo "TheNetwork usage:"
grep -r "TheNetwork" Source Include --include="*.cpp" --include="*.h" | \
    grep -v "/GameNetwork/" | \
    cut -d: -f1 | sort -u | head -10

echo ""
echo "TheLAN usage:"
grep -r "TheLAN" Source Include --include="*.cpp" --include="*.h" | \
    grep -v "/GameNetwork/" | \
    cut -d: -f1 | sort -u | head -10

echo ""
echo "TheGameInfo usage:"
grep -r "TheGameInfo" Source Include --include="*.cpp" --include="*.h" | \
    grep -v "/GameNetwork/" | \
    cut -d: -f1 | sort -u | head -10

echo ""
echo "TheConnectionManager usage:"
grep -r "TheConnectionManager" Source Include --include="*.cpp" --include="*.h" | \
    grep -v "/GameNetwork/" | \
    cut -d: -f1 | sort -u | head -10

echo ""
echo "=========================================================="
echo "2. Analyzing which GameNetwork headers are included..."
echo ""

# Find all includes of GameNetwork headers
grep -r '#include.*GameNetwork' Source Include --include="*.cpp" --include="*.h" | \
    grep -v "/GameNetwork/" | \
    sed 's/.*#include.*["<]\(.*\)[">].*/\1/' | \
    sort | uniq -c | sort -rn | head -20

echo ""
echo "=========================================================="
echo "3. Finding GameNetwork classes/functions called via global objects..."
echo ""

# Create temp file with non-GameNetwork source files
TEMP_DIR=$(mktemp -d)
trap "rm -rf $TEMP_DIR" EXIT

find Source Include -type f \( -name "*.cpp" -o -name "*.h" \) | \
    grep -v "/GameNetwork/" > "$TEMP_DIR/sources.txt"

# Search for method calls on network objects
echo "Most common network-related method calls:"
cat "$TEMP_DIR/sources.txt" | xargs grep -h "TheNetwork->\|TheLAN->\|TheGameInfo->\|TheConnectionManager->" 2>/dev/null | \
    sed 's/.*->\([a-zA-Z_][a-zA-Z0-9_]*\).*/\1/' | \
    sort | uniq -c | sort -rn | head -20

echo ""
echo "=========================================================="
echo "4. Summary of GameNetwork dependencies..."
echo ""

# Count files that use GameNetwork
TOTAL_FILES=$(wc -l < "$TEMP_DIR/sources.txt")
FILES_USING_NETWORK=$(grep -l "GameNetwork\|TheNetwork\|TheLAN\|TheGameInfo" "$TEMP_DIR/sources.txt" | wc -l)

echo "Total non-GameNetwork source files: $TOTAL_FILES"
echo "Files that reference GameNetwork: $FILES_USING_NETWORK"
echo "Percentage: $(( FILES_USING_NETWORK * 100 / TOTAL_FILES ))%"

echo ""
echo "=========================================================="
echo "5. Key GameNetwork integration points..."
echo ""

# Find the main integration points
echo "Files that create/initialize network objects:"
grep -r "new.*Connection\|new.*Network\|createNetwork\|initNetwork" Source Include --include="*.cpp" | \
    grep -v "/GameNetwork/" | \
    cut -d: -f1 | sort -u | head -10

echo ""
echo "Files that handle network messages:"
grep -r "processNetCommand\|handleGame\|OnGame" Source Include --include="*.cpp" --include="*.h" | \
    grep -v "/GameNetwork/" | \
    cut -d: -f1 | sort -u | head -10

echo ""
echo "=========================================================="
echo "Analysis complete!"