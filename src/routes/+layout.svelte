<script lang="ts">
  // Polices embarquées : l'application ne dépend d'aucun service en ligne pour s'afficher.
  import '@fontsource-variable/archivo/wdth.css';
  import '@fontsource-variable/martian-mono';
  import '../app.css';

  let { children } = $props();
</script>

{@render children()}
