import { _DEFINE_PROG, _DEFINE_VER } from "vars";
import { program } from "@commander-js/extra-typings";
import { addConfigureCommand } from "cmd/configure";
import { addNfoCommand } from "cmd/nfo";
import { addIdmapCommand } from "cmd/idmap";

program
  .name(_DEFINE_PROG)
  .version(_DEFINE_VER)
  .description("Utility to export stub-NFO's for use with jellyfin.");

addConfigureCommand(program);
addNfoCommand(program);
addIdmapCommand(program);

program.parse(process.argv);

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
