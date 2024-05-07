#!/bin/sh

_test () {
    jq . "data-extractor/tmp/mods/1.${1%.0}/run/blocks.json" > a
    jq . "data-extractor/tmp/mods/1.${2%.0}/run/blocks.json" > b
    diff -q a b > /dev/null
    blocks=$?

    jq . "class-parser/out/1.${1%.0}/entities.json" > a
    jq . "class-parser/out/1.${2%.0}/entities.json" > b
    diff -q a b > /dev/null
    entities=$?

    jq . "class-parser/out/1.${1%.0}/block_entities.json" > a
    jq . "class-parser/out/1.${2%.0}/block_entities.json" > b
    diff -q a b > /dev/null
    block_entities=$?

    if [ $blocks -eq 1 ] || [ $entities -eq 1 ] || [ $block_entities -eq 1 ]; then
        any=1
    else
        any=0
    fi

    echo "1.$1 -> 1.$2 :: $blocks $entities $block_entities | $any"

    rm a b
}

_test 14.4 15.0
_test 15.0 15.1
_test 15.1 15.2
_test 15.2 16.0
_test 16.0 16.1
_test 16.1 16.2
_test 16.2 16.3
_test 16.3 16.4
_test 16.4 16.5
_test 16.5 17.0
_test 17.0 17.1
_test 17.1 18.0
_test 18.0 18.1
_test 18.1 18.2
_test 18.2 19.0
_test 19.0 19.1
_test 19.1 19.2
_test 19.2 19.3
_test 19.3 19.4
_test 19.4 20.0
_test 20.0 20.1
_test 20.1 20.2
_test 20.2 20.3
_test 20.3 20.4
_test 20.4 20.5
_test 20.5 20.6
