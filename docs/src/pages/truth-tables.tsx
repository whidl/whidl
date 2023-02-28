import React, { useEffect, useState, useRef } from "react";
import Grid from "@mui/material/Grid";
import init, { full_table } from "@whidl/whidl";
import Alert, { AlertColor } from "@mui/material/Alert";
import TextField from "@mui/material/TextField";
import Table from "@mui/material/Table";
import TableBody from "@mui/material/TableBody";
import TableCell from "@mui/material/TableCell";
import TableContainer from "@mui/material/TableContainer";
import TableHead from "@mui/material/TableHead";
import TableRow from "@mui/material/TableRow";

import * as monaco from 'monaco-editor';
import Editor from "@monaco-editor/react";

let chips = {
  And: 
  `/*
Parts available for use: 
  Nand(a, b, out)
  And(a, b, out) 
  Not(in, out)
  Or (a, b, out)
Chip names are case sensitive.
Only one chip can be created in this text field.
*/

CHIP And {
  IN a, b;
  OUT out;

  PARTS:
  Nand(a=a, b=b, out=nandout);
  Not(in=nandout, out=out);
}`,

  Not: `CHIP Not {
        IN in;
        OUT out;

        PARTS:
        Nand(a=in, b=in, out=out);
      }`,

  Or: `CHIP Or {
      IN a, b;
      OUT out;

      PARTS:
      Not(in=a, out=nota);
      Not(in=b, out=notb);
      Nand(a=nota, b=notb, out=out);
      }`,
  Mux: `/**
 * Multiplexor:
 * out = a if sel == 0
 *       b otherwise
 */

CHIP Mux {
    IN a, b, sel;
    OUT out;

    PARTS:

    Not(in=sel, out=Notsel);

    And(a=a, b=Notsel, out=NotselAnda);
    And(a=b, b=sel, out=selAndb);

    Or(a=NotselAnda, b=selAndb, out=out);
}`,
'Mux4Way': `
CHIP Mux4Way {
    IN a, b, c, d, sel[2];
    OUT out;

    PARTS:
    Mux(a=a, b=b, sel=sel[0], out=outab);
    Mux(a=c, b=d, sel=sel[0], out=outcd);
    Mux(a=outab, b=outcd, sel=sel[1], out=out);
    
}`

};

export default function TruthTableGenerator() {
  // This is only created because a null reference throws an error
  const editor_ref = useRef(monaco.editor.create(document.createElement("editor")));


  type option_bool = boolean | null;

  function handleEditorMount(editor: any) {
    editor_ref.current = editor;
    editor_ref.current.updateOptions({minimap: {enabled: false}});
  }

  function ob_str(v: option_bool) {
    if (v === true) {
      return 1;
    } else if (v === false) {
      return 0;
    } else if (v === null) {
      return "?";
    }
  }

  let initialState: [Array<string>, Array<Array<Array<boolean | null>>>] = [
    [],
    [],
  ];

  const [ans, setAns] = useState(initialState);
  const [status, setStatus] = useState("Ready");
  const [severity, setSeverity] = useState<AlertColor>("success");

  useEffect(() => {
    init().then(() => {
      let table_json = full_table(chips["And"]);
      let table: [Array<string>, Array<Array<Array<option_bool>>>] =
        JSON.parse(table_json);
      console.log(table);
      setAns(table);
    });
  }, []);

  function changeCode(s: any) {
    try {
      let table_json = full_table(s);
      let table: [Array<string>, Array<Array<Array<option_bool>>>] =
        JSON.parse(table_json);
      setAns(table);
      setStatus("OK");
      setSeverity("success");
    } catch (e: any) {
      setStatus(String(e));
      setSeverity("error");
    }
  }

  return (
    <Grid container spacing={{ xs: 2, md: 3}}>
      <Grid item xs={6} sx={{
        left:1
      }}>
        <Editor
          height="70vh"
          width="60vh"
          theme="vs-dark"
          onMount={handleEditorMount}
          onChange={(s) => { changeCode(editor_ref.current.getValue()) }}
          defaultValue={chips["And"]}
        />
      </Grid>
      <Grid item xs={6} spacing={3}>
        <TableContainer sx={{ maxHeight: 440 }}>
          <Table size="small" stickyHeader>
            <TableHead>
              <TableRow>
                {ans[0].map((port_name, index) => {
                  return <TableCell key={index}>{port_name}</TableCell>;
                })}
              </TableRow>
            </TableHead>
            <TableBody>
              {ans[1].map((row, index) => {
                return (
                  <TableRow key={index}>
                    {row.map((column, index) => {
                      return (
                        <TableCell key={index}>{column.map(ob_str)}</TableCell>
                      );
                    })}
                  </TableRow>
                );
              })}
            </TableBody>
          </Table>
        </TableContainer>
      </Grid>
      <Grid item xs={12}>
        <Alert variant="outlined" severity={severity}>
          {status}
        </Alert>
      </Grid>
    </Grid>
  );
}
