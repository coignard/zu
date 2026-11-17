#!/bin/bash
K=${1:-${ZU:-$(cd "$(dirname "$0")/../.." && pwd)/target/release/zu}}; S="${ZUTEST_WORK:-${TMPDIR:-/tmp}/zu-parity}/scrollback"; mkdir -p "$S"
T="tmux -L scrtest"
printf 'GEHEIMMARKER Zeile %d\n' $(seq 1 30) > $S/geheim.txt
$T kill-server 2>/dev/null
$T new-session -d -s a -x 80 -y 24 "bash --norc"; sleep 0.4
$T send-keys -t a "printf 'verlauf %s\\n' \$(seq 1 60)" Enter; sleep 0.4
$T send-keys -t a "$K -I $S/geheim.txt" Enter; sleep 0.8
$T send-keys -t a C-l; sleep 0.3; $T send-keys -t a C-x; sleep 0.5
H=$($T capture-pane -p -S -500 -t a)
KEPT=$(echo "$H" | grep -c '^verlauf '); LEAK_A=$(echo "$H" | grep -c GEHEIMMARKER)
echo "tmux: Verlaufszeilen erhalten: $KEPT/60, Klartext im Verlauf/Bild: $LEAK_A"
rm -f $S/hc.txt; printf 'defscrollback 1000\n' > $S/screenrc
$T new-session -d -s b -x 80 -y 24 "screen -c $S/screenrc -S kt bash --norc"; sleep 0.8
$T send-keys -t b "$K -I $S/geheim.txt" Enter; sleep 0.8
$T send-keys -t b C-l; sleep 0.3; $T send-keys -t b C-x; sleep 0.5
screen -S kt -X hardcopy -h $S/hc.txt; sleep 0.5
LEAK_B=$(grep -c GEHEIMMARKER $S/hc.txt 2>/dev/null)
echo "screen: Klartext im screen-Verlauf: $LEAK_B"
screen -S kt -X quit 2>/dev/null; $T kill-server 2>/dev/null
if [ "$KEPT" = 60 ] && [ "$LEAK_A" = 0 ] && [ "$LEAK_B" = 0 ]; then echo "ok   Verlauf des Terminals"; else echo "FEHL Verlauf des Terminals"; exit 1; fi
