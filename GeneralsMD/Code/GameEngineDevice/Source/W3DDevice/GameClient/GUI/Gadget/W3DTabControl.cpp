// FILE: W3DTabControl.cpp ///////////////////////////////////////////////////
//
// Project:   RTS3
//
// File name: projects\RTS\code\gameenginedevice\Source\W3DDevice\GameClient\GUI\Gadget\W3DTabControl.cpp
//
// Created:   Graham Smallwood, November 2001
//
// Desc:      W3D methods needed to implement the TabControl UI control
//
//-----------------------------------------------------------------------------
///////////////////////////////////////////////////////////////////////////////

// SYSTEM INCLUDES ////////////////////////////////////////////////////////////
#include <stdlib.h>

// USER INCLUDES //////////////////////////////////////////////////////////////
#include "GameClient/GameWindowGlobal.h"
#include "GameClient/GameWindowManager.h"
#include "GameClient/GadgetTabControl.h"
#include "W3DDevice/GameClient/W3DGameWindow.h"
#include "W3DDevice/GameClient/W3DGadget.h"
#include "W3DDevice/GameClient/W3DDisplay.h"

// DEFINES ////////////////////////////////////////////////////////////////////

// PRIVATE TYPES //////////////////////////////////////////////////////////////

// PRIVATE DATA ///////////////////////////////////////////////////////////////

// PUBLIC DATA ////////////////////////////////////////////////////////////////

// PRIVATE PROTOTYPES /////////////////////////////////////////////////////////

// PRIVATE FUNCTIONS //////////////////////////////////////////////////////////

///////////////////////////////////////////////////////////////////////////////
// PUBLIC FUNCTIONS ///////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////

// W3DGadgetRadioButtonDraw ===================================================
/** Draw tabs with standard graphics */
//=============================================================================
void W3DGadgetTabControlDraw( GameWindow *tabControl, WinInstanceData *instData )
{
	ICoord2D origin, size;

	// get window position and size
	tabControl->winGetScreenPosition( &origin.x, &origin.y );
	tabControl->winGetSize( &size.x, &size.y );

	W3DGameWinDefaultDraw(tabControl, instData);//draw the background

	if( BitTest( tabControl->winGetStatus(), WIN_STATUS_BORDER ) == TRUE &&
			!BitTest( tabControl->winGetStatus(), WIN_STATUS_SEE_THRU ) )
	{//draw border if desired
		tabControl->winDrawBorder();
	}

	TabControlData *tabData = (TabControlData *)tabControl->winGetUserData();

	Int tabX, tabY, tabWidth, tabHeight, tabDeltaX, tabDeltaY;
	tabX = origin.x + tabData->tabsLeftLimit;
	tabY = origin.y + tabData->tabsTopLimit;
	tabWidth = tabData->tabWidth;
	tabHeight = tabData->tabHeight;
	if( (tabData->tabEdge == TP_TOP_SIDE)  ||  (tabData->tabEdge == TP_BOTTOM_SIDE) )
	{
		tabDeltaX = tabWidth;
		tabDeltaY = 0;
	}
	else
	{
		tabDeltaX = 0;
		tabDeltaY = tabHeight;
	}

	Color color, border;

	if( tabData->tabCount >= 1 )//Does exist
	{
		if( tabData->subPaneDisabled[0] )
		{//Disabled
			color			= GadgetTabControlGetDisabledColorTabZero( tabControl );
			border		= GadgetTabControlGetDisabledBorderColorTabZero( tabControl );
		}  
		else if( tabData->activeTab == 0 )
		{//Hilited/Active
			color			= GadgetTabControlGetHiliteColorTabZero( tabControl );
			border		= GadgetTabControlGetHiliteBorderColorTabZero( tabControl );
		}  
		else
		{//Just enabled
			color			= GadgetTabControlGetEnabledColorTabZero( tabControl );
			border		= GadgetTabControlGetEnabledBorderColorTabZero( tabControl );
		} 

		// box and border
		if( border != WIN_COLOR_UNDEFINED )
		{
			TheWindowManager->winOpenRect( border, WIN_DRAW_LINE_WIDTH,
																		 tabX, tabY, tabX + tabWidth, tabY + tabHeight );
		}
		if( color != WIN_COLOR_UNDEFINED )
		{
			TheWindowManager->winFillRect( color, WIN_DRAW_LINE_WIDTH,
																		 tabX + 1, tabY + 1, tabX + tabWidth - 1, tabY + tabHeight - 1 );
		}
	}

	tabX += tabDeltaX;
	tabY += tabDeltaY;

	if( tabData->tabCount >= 2 )//Does exist
	{
		if( tabData->subPaneDisabled[1] )
		{//Disabled
			color			= GadgetTabControlGetDisabledColorTabOne( tabControl );
			border		= GadgetTabControlGetDisabledBorderColorTabOne( tabControl );
		}  
		else if( tabData->activeTab == 1 )
		{//Hilited/Active
			color			= GadgetTabControlGetHiliteColorTabOne( tabControl );
			border		= GadgetTabControlGetHiliteBorderColorTabOne( tabControl );
		}  
		else
		{//Just enabled
			color			= GadgetTabControlGetEnabledColorTabOne( tabControl );
			border		= GadgetTabControlGetEnabledBorderColorTabOne( tabControl );
		} 

		// box and border
		if( border != WIN_COLOR_UNDEFINED )
		{
			TheWindowManager->winOpenRect( border, WIN_DRAW_LINE_WIDTH,
																		 tabX, tabY, tabX + tabWidth, tabY + tabHeight );
		}
		if( color != WIN_COLOR_UNDEFINED )
		{
			TheWindowManager->winFillRect( color, WIN_DRAW_LINE_WIDTH,
																		 tabX + 1, tabY + 1, tabX + tabWidth - 1, tabY + tabHeight - 1 );
		}
	}

	tabX += tabDeltaX;
	tabY += tabDeltaY;

	if( tabData->tabCount >= 3 )//Does exist
	{
		if( tabData->subPaneDisabled[2] )
		{//Disabled
			color			= GadgetTabControlGetDisabledColorTabTwo( tabControl );
			border		= GadgetTabControlGetDisabledBorderColorTabTwo( tabControl );
		}  
		else if( tabData->activeTab == 2 )
		{//Hilited/Active
			color			= GadgetTabControlGetHiliteColorTabTwo( tabControl );
			border		= GadgetTabControlGetHiliteBorderColorTabTwo( tabControl );
		}  
		else
		{//Just enabled
			color			= GadgetTabControlGetEnabledColorTabTwo( tabControl );
			border		= GadgetTabControlGetEnabledBorderColorTabTwo( tabControl );
		} 

		// box and border
		if( border != WIN_COLOR_UNDEFINED )
		{
			TheWindowManager->winOpenRect( border, WIN_DRAW_LINE_WIDTH,
																		 tabX, tabY, tabX + tabWidth, tabY + tabHeight );
		}
		if( color != WIN_COLOR_UNDEFINED )
		{
			TheWindowManager->winFillRect( color, WIN_DRAW_LINE_WIDTH,
																		 tabX + 1, tabY + 1, tabX + tabWidth - 1, tabY + tabHeight - 1 );
		}
	}

	tabX += tabDeltaX;
	tabY += tabDeltaY;

	if( tabData->tabCount >= 4 )//Does exist
	{
		if( tabData->subPaneDisabled[3] )
		{//Disabled
			color			= GadgetTabControlGetDisabledColorTabThree( tabControl );
			border		= GadgetTabControlGetDisabledBorderColorTabThree( tabControl );
		}  
		else if( tabData->activeTab == 3 )
		{//Hilited/Active
			color			= GadgetTabControlGetHiliteColorTabThree( tabControl );
			border		= GadgetTabControlGetHiliteBorderColorTabThree( tabControl );
		}  
		else
		{//Just enabled
			color			= GadgetTabControlGetEnabledColorTabThree( tabControl );
			border		= GadgetTabControlGetEnabledBorderColorTabThree( tabControl );
		} 

		// box and border
		if( border != WIN_COLOR_UNDEFINED )
		{
			TheWindowManager->winOpenRect( border, WIN_DRAW_LINE_WIDTH,
																		 tabX, tabY, tabX + tabWidth, tabY + tabHeight );
		}
		if( color != WIN_COLOR_UNDEFINED )
		{
			TheWindowManager->winFillRect( color, WIN_DRAW_LINE_WIDTH,
																		 tabX + 1, tabY + 1, tabX + tabWidth - 1, tabY + tabHeight - 1 );
		}
	}

	tabX += tabDeltaX;
	tabY += tabDeltaY;

	if( tabData->tabCount >= 5 )//Does exist
	{
		if( tabData->subPaneDisabled[4] )
		{//Disabled
			color			= GadgetTabControlGetDisabledColorTabFour( tabControl );
			border		= GadgetTabControlGetDisabledBorderColorTabFour( tabControl );
		}  
		else if( tabData->activeTab == 4 )
		{//Hilited/Active
			color			= GadgetTabControlGetHiliteColorTabFour( tabControl );
			border		= GadgetTabControlGetHiliteBorderColorTabFour( tabControl );
		}  
		else
		{//Just enabled
			color			= GadgetTabControlGetEnabledColorTabFour( tabControl );
			border		= GadgetTabControlGetEnabledBorderColorTabFour( tabControl );
		} 

		// box and border
		if( border != WIN_COLOR_UNDEFINED )
		{
			TheWindowManager->winOpenRect( border, WIN_DRAW_LINE_WIDTH,
																		 tabX, tabY, tabX + tabWidth, tabY + tabHeight );
		}
		if( color != WIN_COLOR_UNDEFINED )
		{
			TheWindowManager->winFillRect( color, WIN_DRAW_LINE_WIDTH,
																		 tabX + 1, tabY + 1, tabX + tabWidth - 1, tabY + tabHeight - 1 );
		}
	}

	tabX += tabDeltaX;
	tabY += tabDeltaY;

	if( tabData->tabCount >= 6 )//Does exist
	{
		if( tabData->subPaneDisabled[5] )
		{//Disabled
			color			= GadgetTabControlGetDisabledColorTabFive( tabControl );
			border		= GadgetTabControlGetDisabledBorderColorTabFive( tabControl );
		}  
		else if( tabData->activeTab == 5 )
		{//Hilited/Active
			color			= GadgetTabControlGetHiliteColorTabFive( tabControl );
			border		= GadgetTabControlGetHiliteBorderColorTabFive( tabControl );
		}  
		else
		{//Just enabled
			color			= GadgetTabControlGetEnabledColorTabFive( tabControl );
			border		= GadgetTabControlGetEnabledBorderColorTabFive( tabControl );
		} 

		// box and border
		if( border != WIN_COLOR_UNDEFINED )
		{
			TheWindowManager->winOpenRect( border, WIN_DRAW_LINE_WIDTH,
																		 tabX, tabY, tabX + tabWidth, tabY + tabHeight );
		}
		if( color != WIN_COLOR_UNDEFINED )
		{
			TheWindowManager->winFillRect( color, WIN_DRAW_LINE_WIDTH,
																		 tabX + 1, tabY + 1, tabX + tabWidth - 1, tabY + tabHeight - 1 );
		}
	}

	tabX += tabDeltaX;
	tabY += tabDeltaY;

	if( tabData->tabCount >= 7 )//Doesn't exist
	{
		if( tabData->subPaneDisabled[6] )
		{//Disabled
			color			= GadgetTabControlGetDisabledColorTabSix( tabControl );
			border		= GadgetTabControlGetDisabledBorderColorTabSix( tabControl );
		}  
		else if( tabData->activeTab == 6 )
		{//Hilited/Active
			color			= GadgetTabControlGetHiliteColorTabSix( tabControl );
			border		= GadgetTabControlGetHiliteBorderColorTabSix( tabControl );
		}  
		else
		{//Just enabled
			color			= GadgetTabControlGetEnabledColorTabSix( tabControl );
			border		= GadgetTabControlGetEnabledBorderColorTabSix( tabControl );
		} 

		// box and border
		if( border != WIN_COLOR_UNDEFINED )
		{
			TheWindowManager->winOpenRect( border, WIN_DRAW_LINE_WIDTH,
																		 tabX, tabY, tabX + tabWidth, tabY + tabHeight );
		}
		if( color != WIN_COLOR_UNDEFINED )
		{
			TheWindowManager->winFillRect( color, WIN_DRAW_LINE_WIDTH,
																		 tabX + 1, tabY + 1, tabX + tabWidth - 1, tabY + tabHeight - 1 );
		}
	}

	tabX += tabDeltaX;
	tabY += tabDeltaY;

	if( tabData->tabCount >= 8 )//Doesn't exist
	{
		if( tabData->subPaneDisabled[7] )
		{//Disabled
			color			= GadgetTabControlGetDisabledColorTabSeven( tabControl );
			border		= GadgetTabControlGetDisabledBorderColorTabSeven( tabControl );
		}  
		else if( tabData->activeTab == 7 )
		{//Hilited/Active
			color			= GadgetTabControlGetHiliteColorTabSeven( tabControl );
			border		= GadgetTabControlGetHiliteBorderColorTabSeven( tabControl );
		}  
		else
		{//Just enabled
			color			= GadgetTabControlGetEnabledColorTabSeven( tabControl );
			border		= GadgetTabControlGetEnabledBorderColorTabSeven( tabControl );
		} 

		// box and border
		if( border != WIN_COLOR_UNDEFINED )
		{
			TheWindowManager->winOpenRect( border, WIN_DRAW_LINE_WIDTH,
																		 tabX, tabY, tabX + tabWidth, tabY + tabHeight );
		}
		if( color != WIN_COLOR_UNDEFINED )
		{
			TheWindowManager->winFillRect( color, WIN_DRAW_LINE_WIDTH,
																		 tabX + 1, tabY + 1, tabX + tabWidth - 1, tabY + tabHeight - 1 );
		}
	}

}  // end W3DGadgetTabControlDraw

// W3DGadgetRadioButtonImageDraw ==============================================
/** Draw tabs with user supplied images */
//=============================================================================
void W3DGadgetTabControlImageDraw( GameWindow *tabControl, 
																	WinInstanceData *instData )
{
	ICoord2D origin, size;

	// get window position and size
	tabControl->winGetScreenPosition( &origin.x, &origin.y );
	tabControl->winGetSize( &size.x, &size.y );

	W3DGameWinDefaultDraw(tabControl, instData);//draw the background

	if( BitTest( tabControl->winGetStatus(), WIN_STATUS_BORDER ) == TRUE &&
			!BitTest( tabControl->winGetStatus(), WIN_STATUS_SEE_THRU ) )
	{//draw border if desired
		tabControl->winDrawBorder();
	}

	TabControlData *tabData = (TabControlData *)tabControl->winGetUserData();

	Int tabX, tabY, tabWidth, tabHeight, tabDeltaX, tabDeltaY;
	tabX = origin.x + tabData->tabsLeftLimit;
	tabY = origin.y + tabData->tabsTopLimit;
	tabWidth = tabData->tabWidth;
	tabHeight = tabData->tabHeight;
	if( (tabData->tabEdge == TP_TOP_SIDE)  ||  (tabData->tabEdge == TP_BOTTOM_SIDE) )
	{
		tabDeltaX = tabWidth;
		tabDeltaY = 0;
	}
	else
	{
		tabDeltaX = 0;
		tabDeltaY = tabHeight;
	}

	const Image *image = NULL;

	if( tabData->tabCount >= 1 )//Does exist
	{
		if( tabData->subPaneDisabled[0] )
		{//Disabled
			image			= GadgetTabControlGetDisabledImageTabZero( tabControl );
		}  
		else if( tabData->activeTab == 0 )
		{//Hilited/Active
			image			= GadgetTabControlGetHiliteImageTabZero( tabControl );
		}  
		else
		{//Just enabled
			image			= GadgetTabControlGetEnabledImageTabZero( tabControl );
		} 

		if( image != NULL )
		{
			TheWindowManager->winDrawImage( image,
																			tabX,
																			tabY,
																			tabX + tabWidth,
																			tabY + tabHeight 
																			);
		}
	}

	tabX += tabDeltaX;
	tabY += tabDeltaY;

	if( tabData->tabCount >= 2 )//Does exist
	{
		if( tabData->subPaneDisabled[1] )
		{//Disabled
			image			= GadgetTabControlGetDisabledImageTabOne( tabControl );
		}  
		else if( tabData->activeTab == 1 )
		{//Hilited/Active
			image			= GadgetTabControlGetHiliteImageTabOne( tabControl );
		}  
		else
		{//Just enabled
			image			= GadgetTabControlGetEnabledImageTabOne( tabControl );
		} 

		if( image != NULL )
		{
			TheWindowManager->winDrawImage( image,
																			tabX,
																			tabY,
																			tabX + tabWidth,
																			tabY + tabHeight 
																			);
		}
	}

	tabX += tabDeltaX;
	tabY += tabDeltaY;

	if( tabData->tabCount >= 3 )//Does exist
	{
		if( tabData->subPaneDisabled[2] )
		{//Disabled
			image			= GadgetTabControlGetDisabledImageTabTwo( tabControl );
		}  
		else if( tabData->activeTab == 2 )
		{//Hilited/Active
			image			= GadgetTabControlGetHiliteImageTabTwo( tabControl );
		}  
		else
		{//Just enabled
			image			= GadgetTabControlGetEnabledImageTabTwo( tabControl );
		} 

		if( image != NULL )
		{
			TheWindowManager->winDrawImage( image,
																			tabX,
																			tabY,
																			tabX + tabWidth,
																			tabY + tabHeight 
																			);
		}
	}

	tabX += tabDeltaX;
	tabY += tabDeltaY;

	if( tabData->tabCount >= 4 )//Does exist
	{
		if( tabData->subPaneDisabled[3] )
		{//Disabled
			image			= GadgetTabControlGetDisabledImageTabThree( tabControl );
		}  
		else if( tabData->activeTab == 3 )
		{//Hilited/Active
			image			= GadgetTabControlGetHiliteImageTabThree( tabControl );
		}  
		else
		{//Just enabled
			image			= GadgetTabControlGetEnabledImageTabThree( tabControl );
		} 

		if( image != NULL )
		{
			TheWindowManager->winDrawImage( image,
																			tabX,
																			tabY,
																			tabX + tabWidth,
																			tabY + tabHeight 
																			);
		}
	}

	tabX += tabDeltaX;
	tabY += tabDeltaY;

	if( tabData->tabCount >= 5 )//Does exist
	{
		if( tabData->subPaneDisabled[4] )
		{//Disabled
			image			= GadgetTabControlGetDisabledImageTabFour( tabControl );
		}  
		else if( tabData->activeTab == 4 )
		{//Hilited/Active
			image			= GadgetTabControlGetHiliteImageTabFour( tabControl );
		}  
		else
		{//Just enabled
			image			= GadgetTabControlGetEnabledImageTabFour( tabControl );
		} 

		if( image != NULL )
		{
			TheWindowManager->winDrawImage( image,
																			tabX,
																			tabY,
																			tabX + tabWidth,
																			tabY + tabHeight 
																			);
		}
	}

	tabX += tabDeltaX;
	tabY += tabDeltaY;

	if( tabData->tabCount >= 6 )//Does exist
	{
		if( tabData->subPaneDisabled[5] )
		{//Disabled
			image			= GadgetTabControlGetDisabledImageTabFive( tabControl );
		}  
		else if( tabData->activeTab == 5 )
		{//Hilited/Active
			image			= GadgetTabControlGetHiliteImageTabFive( tabControl );
		}  
		else
		{//Just enabled
			image			= GadgetTabControlGetEnabledImageTabFive( tabControl );
		} 

		if( image != NULL )
		{
			TheWindowManager->winDrawImage( image,
																			tabX,
																			tabY,
																			tabX + tabWidth,
																			tabY + tabHeight 
																			);
		}
	}

	tabX += tabDeltaX;
	tabY += tabDeltaY;

	if( tabData->tabCount >= 7 )//Doesn't exist
	{
		if( tabData->subPaneDisabled[6] )
		{//Disabled
			image			= GadgetTabControlGetDisabledImageTabSix( tabControl );
		}  
		else if( tabData->activeTab == 6 )
		{//Hilited/Active
			image			= GadgetTabControlGetHiliteImageTabSix( tabControl );
		}  
		else
		{//Just enabled
			image			= GadgetTabControlGetEnabledImageTabSix( tabControl );
		} 

		if( image != NULL )
		{
			TheWindowManager->winDrawImage( image,
																			tabX,
																			tabY,
																			tabX + tabWidth,
																			tabY + tabHeight 
																			);
		}
	}

	tabX += tabDeltaX;
	tabY += tabDeltaY;

	if( tabData->tabCount >= 8 )//Doesn't exist
	{
		if( tabData->subPaneDisabled[7] )
		{//Disabled
			image			= GadgetTabControlGetDisabledImageTabSeven( tabControl );
		}  
		else if( tabData->activeTab == 7 )
		{//Hilited/Active
			image			= GadgetTabControlGetHiliteImageTabSeven( tabControl );
		}  
		else
		{//Just enabled
			image			= GadgetTabControlGetEnabledImageTabSeven( tabControl );
		} 

		if( image != NULL )
		{
			TheWindowManager->winDrawImage( image,
																			tabX,
																			tabY,
																			tabX + tabWidth,
																			tabY + tabHeight 
																			);
		}
	}

}  // end W3DGadgetTabControlImageDraw
