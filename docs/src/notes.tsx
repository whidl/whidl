// The main content area can be one of two things - <Notes> or <SlideShow>

import React, { useEffect } from 'react';
import Box from '@mui/material/Box';

interface NotesProps {
    children: React.ReactNode
}

// Slideshow needs to map over Children in order to use convenient
// nesting in MDX files. Because I am using MDX for the slideshow
// it doesn't seem like any of the alternatives to Children
// listed in the beta React docs are a good fit. We wil keep the use
// of Children to a minimum here - only used to hide and show slides.

// A Slideshow must consist of only Slide children.
// Nested slides are not supported.
// Custom components that generate Slide components are not supported.
export default function Notes(props: NotesProps) {
    let children = props.children;

    return (
        <React.Fragment>
        <Box sx={{width: '768px', margin: 'auto'}}>{children}</Box>
        </React.Fragment>
    );

}
