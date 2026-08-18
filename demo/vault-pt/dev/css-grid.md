# Grid e alinhamento

## Quando usar grid e quando usar flex

Grid é para duas dimensões, flex é para uma. Se você está escrevendo
`flex-wrap` e calculando larguras na mão, provavelmente queria grid.

## Colunas que se adaptam sozinhas

`repeat(auto-fit, minmax(220px, 1fr))` cria quantas colunas couberem sem
media query nenhuma. É o truque que mais me poupou linhas de CSS.

## Espaçamento

`gap` em vez de margem nos filhos. Margem no último item cria um espaço
sobrando que alguém depois vai tentar remover com um seletor especial.
