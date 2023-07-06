import React from 'react'
import ReactDOM from "react-dom/client";
import { StyledEngineProvider } from "@mui/material/styles";
import { HashRouter, Routes, Route } from "react-router-dom";
import App from "./app";
import Notes from "./notes";

import Home from "./pages/home.mdx";
import Assignment from "./pages/assignment.mdx";
import Generics from "./pages/generics.mdx";
import Loops from "./pages/loops.mdx";
import Usage from "./pages/usage.mdx";
import TruthTables from "./pages/truth-tables";
import SynthVHDL from "./pages/synth-vhdl.mdx";

ReactDOM.createRoot(document.querySelector("#root")!).render(
    <React.StrictMode>
        <StyledEngineProvider injectFirst>
            <HashRouter>
                <Routes>
                    <Route path="/" element={<App />}>
                        <Route index element={<Home />} />
                        <Route path="assignment" element={<Notes> <Assignment /> </Notes>} />
                        <Route path="generics" element={<Notes> <Generics /> </Notes>} />
                        <Route path="loops" element={<Notes> <Loops /> </Notes>} />
                        <Route path="usage" element={<Notes> <Usage /> </Notes>} />
                        <Route path="truth-tables" element={<Notes> <TruthTables /> </Notes>} />
                        <Route path="synth-vhdl" element={<Notes> <SynthVHDL /> </Notes>} />
                    </Route>
                </Routes>
            </HashRouter>
        </StyledEngineProvider>
    </React.StrictMode>
)
