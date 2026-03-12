#if defined(_MSC_VER)
#pragma once
#endif

#ifndef WW3DTRIG_H
#define WW3DTRIG_H


/*
** The WW3D Library will check for debugging triggers using the following ID's
** Your application may install a trigger handler into the WWDebug library and then
** watch for these ID's coming through.  Then if you wish to "trigger" one, then 
** assign a key or something to the trigger and when your trigger handler is called,
** check if the key is down.  The trigger can be enabled in any way you want but
** in practice, most things will probably be tied to keys.
*/
enum 
{
	WW3D_TRIGGER_RENDER_STATS =			0x100,		// display render stats in the debug window
	WW3D_TRIGGER_SURFACE_CACHE_STATS =	0x101,		// display surface cache info in the debug window
	WW3D_TRIGGER_PROCESS_STATS =			0x102			// render stats for last frame only
};

#endif