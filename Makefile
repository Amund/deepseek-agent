# NOTE: les arguments précédés par des moins (-y, --version) seront capturés par make et ne seront pas disponibles pour les commandes
#
# On peut les forcer en ajoutant un argument "--" :
# Tout ce qui suit cet argument spécial n'est pas capturé par make, et sera donc correctement envoyé vers les commandes
#
# La notation générique permet de complètement contourner ce problème : make [action] -- [arguments]
# exemples :
#   make drush -- cim -y
#   make npm -- install malib --save-dev


## GESTION DES CONTAINERS
.PHONY: up down shell shell-root logs

up: # Start up containers
	@docker compose up -d --remove-orphans
down: # Stop containers
	@docker compose down -v
shell:
	@docker compose exec app /bin/bash
shell-root:
	@docker compose exec -u 0:0 app /bin/bash
logs:
	@docker compose logs -f


## OUTILS COURANTS
.PHONY: composer wp phpstan phpunit lightningcss

composer:
	@docker compose exec app composer $(filter-out $@,$(MAKECMDGOALS)) || true
wp:
	@docker compose exec app wp $(filter-out $@,$(MAKECMDGOALS)) || true
phpstan:
	@docker compose exec app phpstan $(filter-out $@,$(MAKECMDGOALS)) || true
phpunit:
	@docker compose exec app phpunit $(filter-out $@,$(MAKECMDGOALS)) || true
lightningcss:
	@docker compose exec app lightningcss $(filter-out $@,$(MAKECMDGOALS)) || true

%:
	@:
