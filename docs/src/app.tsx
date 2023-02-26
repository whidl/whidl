import React from "react";

// Material UI
import { ThemeProvider, createTheme } from "@mui/material/styles";
import CssBaseline from "@mui/material/CssBaseline";
import Box from "@mui/material/Box";
import Toolbar from "@mui/material/Toolbar";
import AppBar from "@mui/material/AppBar";
import Typography from "@mui/material/Typography";
import Drawer from "@mui/material/Drawer";
import useMediaQuery from "@mui/material/useMediaQuery";

// Icons
import IconButton from "@mui/material/IconButton";
import MenuOpenIcon from '@mui/icons-material/MenuOpen';
import Brightness4Icon from "@mui/icons-material/Brightness4";
import Brightness7Icon from "@mui/icons-material/Brightness7";

import GlobalStyles from '@mui/material/GlobalStyles';
import './index.css'


export default function App() {

    // Light / dark mode
    const colorMode = React.useMemo(
        () => ({
            toggleColorMode: () => {
                setMode((prevMode) => (prevMode === "light" ? "dark" : "light"));
            },
        }),
        []
    );
    const prefersDarkMode = useMediaQuery("(prefers-color-scheme: dark)")
        ? "dark"
        : "light";
    const [mode, setMode] = React.useState<"light" | "dark">(prefersDarkMode);

    // Side menu
    const [drawerOpen, openDrawer] = React.useState(false);
    const toggleDrawer = () => {
        if (drawerOpen) {
            openDrawer(false);
        } else {
            openDrawer(true);
        }
    }

    // Icon buttons that the outlet component wants to add to the nav bar.
    const outletContext = {
        slideShowContext: React.useState(false),
        extraButtonsContext: React.useState<React.ReactNode[]>([]),
    };

    // For sharing state with the main content outlet.
    let contentWidth = outletContext.slideShowContext[0] ? '100%' : '900px';

    const theme = React.useMemo(
        () =>
            createTheme({
                palette: {
                    mode,
                },
            }),
        [mode]
    );

    return (
        <React.Fragment>

            <ThemeProvider theme={theme}>
                <Box sx={{ display: 'flex', height: '100vh' }}>
                    <CssBaseline enableColorScheme />
                    <AppBar
                        position="fixed"
                        sx={{ zIndex: (theme) => theme.zIndex.drawer + 1 }}
                    >
                        <Toolbar>
                            <Typography variant="h6" component="div" sx={{ flexGrow: 1 }}>
                                WHiDL Docs
                            </Typography>

                            {
                                // Buttons added by the outlet child
                                outletContext.extraButtonsContext[0]
                            }

                            <IconButton
                                sx={{ ml: 1 }}
                                onClick={colorMode.toggleColorMode}
                                color="inherit"
                            >
                                {theme.palette.mode === "dark" ? (
                                    <Brightness7Icon />
                                ) : (
                                    <Brightness4Icon />
                                )}
                            </IconButton>
                        </Toolbar>
                    </AppBar>
                    <Drawer variant="permanent" sx={{ width: 250, display: { xs: 'none', lg: 'block' } }} PaperProps={{ sx: { width: 250 } }}>
                        <Toolbar />
                    </Drawer>
                    <Drawer
                        variant="temporary"
                        sx={{ width: 250, display: { xs: 'block', lg: 'none' }, zIndex: 10000 }}
                        PaperProps={{ sx: { width: 250 } }}
                        ModalProps={{
                            keepMounted: true, // Better open performance on mobile.
                        }}
                    >
                        <Typography variant="h6" sx={{ m: 2 }}>
                            WHiDL Docs
                        </Typography>

                    </Drawer>
                    <Box component="main" sx={{ flexGrow: 1, display: 'flex', justifyContent: 'center', mt: '64px', mb: '32px', ml: 4, mr: 4 }}>
                        <Box className="section-to-print" sx={{ flexGrow: 1, maxWidth: contentWidth }}>
                        </Box>
                    </Box>
                </Box>
            </ThemeProvider>
        </React.Fragment>
    )
}
