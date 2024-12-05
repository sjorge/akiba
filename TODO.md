- commands
  - rename
    - cleanup empty directories on old path
    - sanitize dir names (https://github.com/sjorge/adbren/blob/059b67858f797a11a0d633954ecf886b96fd8267/adbren.pl#L297)
    - sanitize file names
      s/[`]/'/g;
      s/[^a-zA-Z0-9-&!`',.~+\- \(\)]/_/g;
      s/[_]+/_/g;
	  s/^_//g;

	  s/[_]+/_/g;
      s/_\./\./g;
      s/_-\./-/g;
      s/_\ /\ /g;
      s/\ _/\ /g;
    - mark as owned
  - batch (for cron where we run a rename + write-nfo on a directory)
