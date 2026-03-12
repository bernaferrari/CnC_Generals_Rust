// FILE: ControlBarPrintPositions.cpp /////////////////////////////////////////////////
//-----------------------------------------------------------------------------
//                                                                          
//                       Electronic Arts Pacific.                          
//                                                                          
//                       Confidential Information                           
//                Copyright (C) 2002 - All Rights Reserved                  
//                                                                          
//-----------------------------------------------------------------------------
//
//	created:	Sep 2002
//
//	Filename: 	ControlBarPrintPositions.cpp
//
//	author:		Chris Huybregts
//	
//	purpose:	Convience function for degayifying the whole squished control bar
//						process
//
//-----------------------------------------------------------------------------
///////////////////////////////////////////////////////////////////////////////

//-----------------------------------------------------------------------------
// SYSTEM INCLUDES ////////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine
#include <stdio.h>
//-----------------------------------------------------------------------------
// USER INCLUDES //////////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------
#include "GameClient/GameWindowManager.h"
#include "GameClient/WindowLayout.h"
//-----------------------------------------------------------------------------
// DEFINES ////////////////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------

//-----------------------------------------------------------------------------
// PUBLIC FUNCTIONS ///////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------
void PrintInfoRecursive( GameWindow *win, FILE *fp)
{
	if(!win)
		return;
	ICoord2D pos, size;
	win->winGetSize(&size.x, &size.y);
	win->winGetPosition(&pos.x, &pos.y);
	fprintf(fp, "ControlBarResizer %s\n",win->winGetInstanceData()->m_decoratedNameString.str());
	fprintf(fp, "  AltPosition = X:%d Y:%d\n",pos.x, pos.y);
	fprintf(fp, "  AltSize = X:%d Y:%d\n",size.x, size.y);
	fprintf(fp, "END\n\n");

	PrintInfoRecursive(win->winGetChild(),fp);
	PrintInfoRecursive(win->winGetNext(),fp);

}

void PrintOffsetsFromControlBarParent( void )
{
	GameWindow *controlBarParent = TheWindowManager->winGetWindowFromId( NULL,  TheNameKeyGenerator->nameToKey( "ControlBar.wnd:ControlBarParent" ));
	if(!controlBarParent)
		return;

	WindowLayout *layout = TheWindowManager->winCreateLayout("controlBarHidden.wnd");
	if(!layout)
		return;
	FILE *fp = fopen("ControlBarEasier.txt", "w");
	if(!fp)
		return;

	PrintInfoRecursive(layout->getFirstWindow(), fp);

	fclose(fp);
	layout->destroyWindows();
	layout->deleteInstance();
}


//-----------------------------------------------------------------------------
// PRIVATE FUNCTIONS //////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------

