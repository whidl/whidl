import React from 'react'
import ReactDOM from "react-dom/client";
import { StyledEngineProvider } from "@mui/material/styles";
import { HashRouter, Routes, Route } from "react-router-dom";
import App from "./app";

import Home from "./pages/home.mdx";
import Assignment from "./pages/assignment.mdx";
import Generics from "./pages/generics.mdx";
import Loops from "./pages/loops.mdx";
import Usage from "./pages/usage.mdx";
import TruthTables from "./pages/truth-tables";

ReactDOM.createRoot(document.querySelector("#root")!).render(
    <React.StrictMode>
        <StyledEngineProvider injectFirst>
            <HashRouter>
                <Routes>
                    <Route path="/" element={<App />}>
                        <Route index element={<Home />} />
                        <Route path="assignment" element={<Assignment />} />
                        <Route path="generics" element={<Generics />} />
                        <Route path="loops" element={<Loops />} />
                        <Route path="usage" element={<Usage />} />
                        <Route path="truth-tables" element={<TruthTables />} />
                    </Route>
                </Routes>
            </HashRouter>
        </StyledEngineProvider>
    </React.StrictMode>
)
