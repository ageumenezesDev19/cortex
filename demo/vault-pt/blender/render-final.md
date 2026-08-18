# Configuração do render final

## Amostras

Cycles com 128 amostras e denoise ligado costuma dar um resultado limpo. Acima
de 256 o ganho é pequeno e o tempo dobra.

## Resolução e proporção

Renderizar em 200% da resolução final e reduzir depois disfarça o serrilhado
melhor do que qualquer filtro de suavização.

## O que quebra no meio da noite

Deixar o render rodando com o disco quase cheio. O Blender escreve os quadros
conforme termina, e quando o espaço acaba ele para sem aviso e sem salvar o
progresso da sequência.
