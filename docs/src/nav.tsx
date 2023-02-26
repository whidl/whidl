import TreeView from "@mui/lab/TreeView";
import ExpandMoreIcon from "@mui/icons-material/ExpandMore";
import TreeItem from "@mui/lab/TreeItem";
import ChevronRightIcon from "@mui/icons-material/ChevronRight";
import { useNavigate } from "react-router-dom";
import React, { useCallback } from "react";

export default function Nav() {

    const navigate = useNavigate();
    const handleOnClick = useCallback(
        (event: React.SyntheticEvent, nodeIds: string) => {
            if (!nodeIds.startsWith("X")) {
                return navigate(nodeIds, { replace: true });
            } else {
                return function() {};
            }
        },
        [navigate]
    );

    // The TreeItem nodeId is used to construct the URL.
    // The TreeItem label is what the user sees.
    // If a nodeId starts with an "X", then click on the node will node
    //  update the main view area. Use this when you want to expand
    //  a menu without creating a link for a new page.
    return (
        <TreeView
            aria-label="classes"
            defaultCollapseIcon={<ExpandMoreIcon />}
            defaultExpandIcon={<ChevronRightIcon />}
            sx={{
                height: 240,
                flexGrow: 1,
                maxWidth: 40025,
                overflowY: "auto",
                m: 2,
                '& .MuiTreeItem-label': { fontSize: '1.25rem !important', },
            }}
            onNodeSelect={handleOnClick}
        >
            <TreeItem nodeId="/" label="Basics" />
            <TreeItem nodeId="assignment" label="Assignments" />
            <TreeItem nodeId="generics" label="Generics" />
            <TreeItem nodeId="loops" label="Loops" />
            <TreeItem nodeId="usage" label="Using whidl" />
            <TreeItem nodeId="truth-tables" label="Truth Table Generator" />
        </TreeView>
    );
}
