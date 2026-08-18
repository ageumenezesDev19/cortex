# Anotações de Rust

## Empréstimo e posse

O compilador não está sendo difícil — está recusando um programa que falharia
em tempo de execução em outra linguagem. Brigar com o borrow checker costuma
ser sinal de que o desenho é que está errado.

## Erros

`Result` em vez de exceção muda como a função é escrita: o caminho que falha
fica visível na assinatura, então quem chama não pode fingir que ele não existe.

## Desempenho

Medir antes de otimizar. Acelerar uma função que representa 2% do tempo total
não muda nada enquanto o gargalo real está numa alocação dentro de um laço.
