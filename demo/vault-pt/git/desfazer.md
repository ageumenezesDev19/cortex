# Desfazer coisas no Git

## Ainda não commitado

`git restore <arquivo>` joga fora a mudança e não tem volta.
`git restore --staged <arquivo>` só tira da área de staging e mantém o texto —
é este que serve na maioria das vezes.

## Commitado, não empurrado

`git commit --amend` conserta o commit de cima: mensagem errada, arquivo
esquecido. `git reset --soft HEAD~1` desmonta o commit e devolve tudo para o
staging, útil quando ele juntou duas coisas que deviam estar separadas.

## Já empurrado

Aí não se apaga, corrige-se por cima. `git revert <commit>` cria um commit novo
que desfaz o antigo, e o histórico continua contando a verdade. Reescrever o
que já está no servidor quebra o clone de quem baixou.

## O que se esquece

`git reflog` guarda para onde o HEAD apontou nos últimos dias. Commit "perdido"
quase nunca está perdido — está lá, sem nome, esperando.
