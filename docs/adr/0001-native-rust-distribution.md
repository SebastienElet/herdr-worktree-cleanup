# 0001 — Runtime Rust et distribution npm native

Statut : décision de préparation locale demandée dans l’issue [#13](https://github.com/SebastienElet/herdr-worktree-cleanup/issues/13), le 2 novembre 2026. Publication et migration de dotfiles exclues.

## Décision

Le traitement de `workspace.closed` devient un binaire Rust. Git refuse les checkouts modifiés, non suivis, verrouillés ou étrangers ; le handler filtre les événements sans worktree, les checkouts absents, non liés ou à HEAD détachée. Les commandes, leur ordre, les messages et `--docker-cleanup=off|stop` sont portés depuis les deux scripts TypeScript, avec l’exception de parité décrite dans « Écart accepté ». Le parseur JSON itératif et la validation reproduisent les diagnostics Bun 1.4.4 / Zod 4.6.7 capturés dans les tests, y compris les entrées incorrectes. `serde_json` sert à sérialiser ces diagnostics ; `tempfile` recueille la sortie Docker sans bloquer un pipe. Chaque commande Docker conserve son délai de 15 secondes et sa terminaison forcée, sans rendre son échec bloquant pour Git.

Le modèle vérifié dans [arnes](https://github.com/SebastienElet/arnes/tree/711582f5cca4eea4350fb0e2581e6f9064ca1855) et [agent-measure](https://github.com/SebastienElet/agent-measure/tree/66f8768c110595e0ec6cf347120e41b15017a872) est repris : paquet npm principal avec lanceur, dépendance optionnelle native à version exacte, métadonnées de build (version, révision, empreinte SHA-256), puis publication native avant le lanceur avec provenance OIDC. La version npm provient de `package.json`. La version de préparation est 1.0.2, après la version TypeScript 1.0.1 ; aucune version n’est publiée ici.

Le lanceur utilise Node standard, sans Bun ni dépendance JavaScript tierce. La seule variante est `darwin-arm64`, cible déjà déclarée par le plugin. Ubuntu sert à tester Rust, sans étendre la distribution à Linux. Le build génère le paquet natif dans `packages/darwin-arm64` ; ses fichiers ne sont pas édités ni versionnés.

Le manifeste canonique `herdr-plugin.toml` est copié dans le paquet natif, à lier dans Herdr. Il exécute `bin/herdr-worktree-cleanup` directement et conserve `dotfiles.worktree-cleanup`, `workspace.closed` et le minimum Herdr 0.9.3. Un manifeste dans le paquet principal dépendrait de l’emplacement choisi par npm pour sa dépendance ; aucun script d’installation ni lien fabriqué après installation n’est retenu.

La CI est configurée pour exécuter fmt, Clippy et les tests Rust sur Ubuntu, les tests Rust et la distribution sur macOS ARM64. Le job `ci` est configuré pour exiger le succès des deux jobs. Les versions Rust et dépendances directes sont exactes ; `Cargo.lock` fixe la résolution. Dependabot est configuré pour surveiller Cargo et les actions avec trois jours de cooldown. Dans le cadre de #13 (« npm si utile »), la décision est de ne pas ajouter de suivi npm : la seule dépendance npm est interne et produite par le même build.

## Livraison ultérieure

Le workflow manuel sur `main` prépare une publication avec `id-token: write` et `npm publish --provenance`, sans secret npm configuré. Les deux paquets devront avoir leur trusted publisher `ci.yml` configuré conformément à la [documentation npm](https://docs.npmjs.com/trusted-publishers/). Si une version existe déjà ou si le résultat est inconnu, vérifier son intégrité et sa provenance avant toute reprise ; le workflow est configuré pour s’arrêter sans remplacement automatique.

Le changement d’installation dans dotfiles et l’essai de fermeture dans Herdr restent des opérations distinctes après merge. Les tests locaux utilisent de vrais dépôts Git jetables et un Docker substitué ; ils ne qualifient pas la livraison d’événements Herdr ni un daemon Docker réel.

## Écart accepté

Sébastien a explicitement accepté cette exception à la parité stricte le 2 novembre 2026 et demandé de conserver la borne de 4 096 niveaux.

Pour une valeur de `length` malformée comportant des tableaux imbriqués, la conversion est itérative et bornée à 4 096 niveaux. Pour une valeur effectivement parsée dépassant cette profondeur pendant la conversion, le handler retourne `failed: Maximum call stack size exceeded.` avec le code 1, sans interruption brutale du processus. Bun 1.4.4 refuse aussi une telle conversion selon sa pile disponible (premier refus observé localement à 4 763 niveaux). Le seuil Rust est déterministe et peut donc différer de Bun. Ce seuil ne limite pas la profondeur du parseur JSON lui-même.
