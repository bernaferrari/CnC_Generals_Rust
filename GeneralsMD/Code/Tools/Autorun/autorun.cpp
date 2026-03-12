#define  STRICT
#include <windows.h>
#include <windowsx.h>
#include <assert.h>
#include <ctype.h>
#include <direct.h>
#include <dos.h>
#include <errno.h>
#include <fcntl.h>
#include <fstream.h>
#include <io.h>
#include <locale.h>
#include <math.h>
#include <mbctype.h>
#include <mmsystem.h>
#include <process.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <strstrea.h>
#include <sys\stat.h>
#include <time.h>
#include <winuser.h>
#include "args.h"
#include "autorun.h"
#include "drawbutton.h"
#include "resource.h"
#include "wnd_file.h" 
//#include "visualc.h"
#include "winfix.h"
#include "cdcntrl.h"
#include "igr.h"
#include "viewhtml.h"

#include "utils.h"
#include "locale_api.h"
//#include "resources.h"
#include "getcd.h"

#include "WSYS_FileSystem.h"
#include "WSYS_STDFileSystem.h"

#include <string>
#include "GameText.h"

#include "leanAndMeanAutorun.h"

#ifndef LEAN_AND_MEAN
#include "Common/SubsystemInterface.h"
#include "GameClient/GameText.h"
#include "Common/UnicodeString.h"
#include "Win32Device/Common/Win32LocalFileSystem.h"
#include "Win32Device/Common/Win32BIGFileSystem.h"
#endif



//-----------------------------------------------------------------------------
//  DEFINES
//-----------------------------------------------------------------------------
#define		PRETEND_ON_CD_TEST				FALSE		// should be FALSE
//#define	PRETEND_ON_CD_TEST				TRUE		// should be FALSE

#define		WINDOW_BRUSH					FALSE
#define		BACKGROUND_BITMAP				TRUE
#define		USE_MOUSE_MOVES					TRUE
#define		DISABLE_KEYBOARD				FALSE

#define		BUTTON_WIDTH 					150
#define		BUTTON_HEIGHT					45
#define		NUM_BUTTONS						10
#define		NUM_ARGUMENTS					10
#define		NUM_SONGS						2		//32 //16
#define		NUM_FLICKER_FRAMES				1
#define		NUM_FLICKER_POSITIONS			15

#define		MOUSE_WAV						"MouseMove"  
#define		SOUND_FILE1						"SPEECH_FILE1"
#define		SOUND_FILE2						"SPEECH_FILE2"

#define		MOH_DEMO_PROGRAM				"MOHAADEMO\\SETUP.EXE"
#define		SHOW_MOH_DEMO					FALSE

#define		BFAVI_FILENAME					"Autorun\\BF1942RTR.avi"
#define		SC4AVI_FILENAME					"Autorun\\Preview.avi"
#define		HELP_FILENAME					"HELP:FILENAME"//"Support\\eahelp.hlp"

#define		SHOW_GAMESPY_BUTTON				FALSE
#define		GAMESPY_WEBSITE					"http://www.gamespyarcade.com/features/launch.asp?svcname=ccrenegade&distID=391"

#define		RESOURCE_FILE					"Autorun.loc"
#define		SETUP_INI_FILE1					"Setup\\Setup.ini"
#define		SETUP_INI_FILE2					"Setup.ini"

#define		UNINSTALL_EXECUTABLE			"IDriver.exe"			// JFS

//-----------------------------------------------------------------------------
// These defines need the Product name from Setup.ini to complete.
//-----------------------------------------------------------------------------
#define		SETUP_MAIN_WINDOW_NAME			"%s Setup" 
#define		CLASS_NAME						"%s Autorun" 
#define		GAME_MAIN_WINDOW_NAME			"%s Game Window" 

//#define	GAME_WEBSITE					"http://www.westwood.com/" 
#define		GAME_WEBSITE					"http://generals.ea.com" 

#define		AUTORUN_MUTEX_OBJECT			"01AF9993-3492-11d3-8F6F-0060089C05B1" 
//#define		GAME_MUTEX_OBJECT			"C6D925A3-7A9B-4ca3-866D-8B4D506C3665" 
#define		GAME_MUTEX_OBJECT				"685EAFF2-3216-4265-B047-251C5F4B82F3"
#define		PRODUCT_VOLUME_CD1	 			"Generals1"
#define		PRODUCT_VOLUME_CD2	 			"Generals2"


//-----------------------------------------------------------------------------
// Global Variables
//-----------------------------------------------------------------------------
LaunchObjectClass	LaunchObject;
MainWindow			*GlobalMainWindow	= NULL;
int					Language			= 0;
int					LanguageToUse		= 0;

DrawButton *ButtonList			[ NUM_BUTTONS ];
RECT 		ButtonSizes			[ NUM_BUTTONS ];
char		ButtonImages		[ NUM_BUTTONS ][ MAX_PATH ];
CHAR		szSongPath			[ MAX_PATH ];
char	 	FocusedButtonImages	[ NUM_BUTTONS ][ MAX_PATH ];
char 		Arguments			[ NUM_ARGUMENTS ][ 30 ];
char	 	szWavs				[ NUM_SONGS][ _MAX_PATH ];
char 		szBuffer 	  		[ MAX_PATH ];
char 		szBuffer1			[ MAX_PATH ];
char 		szBuffer2			[ MAX_PATH ];
char	 	szBuffer3			[ MAX_PATH * 2];
char 		szInternetPath		[_MAX_PATH];
char 		szGamePath			[_MAX_PATH];
char		szWorldbuilderPath	[_MAX_PATH];
char		szPatchgetPath	[_MAX_PATH];
char	 	szSetupPath			[_MAX_PATH];
char 		szUninstallPath		[_MAX_PATH];
char		szUninstallCommandLine[_MAX_PATH];		// JFS: Returned value contains parameters needed.
char	 	szRegisterPath		[_MAX_PATH];
char		szButtonWav	 		[_MAX_PATH ];
char 		szSpeechWav			[_MAX_PATH ];
char	 	szArgvPath			[_MAX_PATH ];
char	 	drive				[_MAX_DRIVE];
char 		dir	 				[_MAX_DIR  ];
char 		szSetupWindow		[_MAX_PATH];
char	 	szGameWindow		[_MAX_PATH];
char	 	szRegistryKey		[_MAX_PATH];
char 		szClassName			[_MAX_PATH];
char 		szVolumeName		[_MAX_PATH];

char		szProduct_Name		[ _MAX_PATH ];


#ifdef LEAN_AND_MEAN

wchar_t 	szWideBuffer   		[ _MAX_PATH ];
wchar_t 	szWideBuffer0  		[ _MAX_PATH ];
wchar_t 	szWideBuffer2  		[ _MAX_PATH ];
wchar_t 	szWideBuffer3  		[ _MAX_PATH ];
wchar_t		szProductName		  [ _MAX_PATH ];
wchar_t		szFullProductName	[ _MAX_PATH ];

//==========================================================================
	// Is WOLAPI DLL installed?
	//==========================================================================
Msg( __LINE__, TEXT(__FILE__), TEXT("---------------- Is_Product_Registered---------------" ));
	Msg( __LINE__, TEXT(__FILE__), TEXT(" InstallProduct		= %d."), InstallProduct	);
	Msg( __LINE__, TEXT(__FILE__), TEXT(" UninstallAvailable	= %d."), UninstallAvailable	);
	Msg( __LINE__, TEXT(__FILE__), TEXT(" IsUserRegistered		= %d."), IsUserRegistered );
	Msg( __LINE__, TEXT(__FILE__), TEXT(" DisplayRegisterButton	= %d."), DisplayRegisterButton );
	Msg( __LINE__, TEXT(__FILE__), TEXT(" szGamePath			= %s."), szGamePath	);
	Msg( __LINE__, TEXT(__FILE__), TEXT(" szSetupPath			= %s."), szSetupPath );
	Msg( __LINE__, TEXT(__FILE__), TEXT(" szRegisterPath		= %s."), szRegisterPath	);
	Msg( __LINE__, TEXT(__FILE__), TEXT(" szInternetPath		= %s."), szInternetPath	);
	Msg( __LINE__, TEXT(__FILE__), TEXT(" szUninstallPath		= %s."), szUninstallPath );

	return( result );
}


/PS_SOLID, 1, TEXT_COLOR );
									HGDIOBJ	oldpen	= SelectObject( hDC, pen );
									SetBkMode( hDC, TRANSPARENT );
					
									MoveToEx(	hDC, BackgroundRect[i].left+1,  BackgroundRect[i].top+1,	NULL );
									LineTo(		hDC, BackgroundRect[i].right-1,	BackgroundRect[i].top+1 ); 
									LineTo(		hDC, BackgroundRect[i].right-1,	BackgroundRect[i].bottom-1 ); 		
									LineTo(		hDC, BackgroundRect[i].left+1,	BackgroundRect[i].bottom-1 ); 		
									LineTo(		hDC, BackgroundRect[i].left+1,	BackgroundRect[i].top+1 );			

									SelectObject( hDC, oldpen );
									DeleteObject( pen );
								#endif
								}
							}
}

					//---------------------------------------------------------------
					// EA_COM Message at the bottom.
					//---------------------------------------------------------------
					#if(0)	// Moved this text to a bitmap.
					fontptr = TTLicenseFontPtr;
					if( fontptr ) {

						if ( b640X480 || b800X600 ) {
							text_rect.X			= 220;
							text_rect.Y			= 400;
							text_rect.Width		= 300;		//460;						 
							text_rect.Height	=  48;		//26;
						} else {
							text_rect.X			= 250;
							text_rect.Y			= 574;
							text_rect.Width		= 420;		//460;						 
							text_rect.Height	=  60;		//26;
						}

						#if(0)
							RECT one;

							one.left	= text_rect.X;
							one.top		= text_rect.Y;
							one.right	= text_rect.X + text_rect.Width;
							one.bottom	= text_rect.Y + text_rect.Height;

							FrameRect( hDC, &one, (HBRUSH)( COLOR_WINDOW + 1 ));
//							DrawFocusRect( hDC, &one );
						#endif

						fontptr->Print( 
							hDC, 
							szWholeString,
							text_rect, 
							TEXT_COLOR, 
							SHADOW_COLOR, 
							TPF_CENTER_TEXT,
							TPF_SHADOW );
					}
					#endif

				#else
					//-------------------------------------------------------------------
					// Select the Brush if it was successfully created.
					//-------------------------------------------------------------------
					if ( hStaticBrush ) {
						HBRUSH  oldBrush   = (HBRUSH) SelectObject( hDC, hStaticBrush );
						GetClientRect( window_handle, (LPRECT) &dlg_rect );
						FillRect( hDC, &dlg_rect, hStaticBrush );
						SelectObject( hDC, oldBrush );
					}
				#endif

//				Msg( __LINE__, TEXT(__FILE__), TEXT("--------------------------------------------------------" ));

				EndPaint( window_handle, &ps );

				//-----------------------------------------------------------------------
				// Play DISK.WAV sound on CD.
				//-----------------------------------------------------------------------
				if ( FirstTime ) { 
					if( UseSounds ) {
						PlaySound( szWavs[ SongNumber ], NULL, SND_ASYNC | SND_RESOURCE );
					}
					FirstTime = FALSE;
				}
			} /* end of if */
			break;

		//-------------------------------------------------------------------------------
		// Background needs to be erased.  Note we are returning 1 here to "fake"
		// Windows into thinking we have already repainted the background with
		// our window brush.  This prevents "flickering" because of background
		// being repainted ( ususally white ) before WM_PAINT is processed.
		//-------------------------------------------------------------------------------
		#if(BACKGROUND_BITMAP)
		case WM_ERASEBKGND:
			InvalidateRect( window_handle, &dlg_rect, FALSE );
			return ( 1 );
		#endif

		//-------------------------------------------------------------------------------
		// Check which button was pressed.  If Explorer button was pressed,
		// call it now so we don't have to exit dialog.
		//-------------------------------------------------------------------------------
		case WM_COMMAND:
			{
				idCtl = LOWORD( w_param );

				unsigned int	result		= TRUE;
				bool			end_dialog	= false;
				int				cd_drive;

				szBuffer[1] = '\0';
				szBuffer[0] = tolower( szArgvPath[0] );
//				cd_drive	= (int)( szBuffer[0] - 'a' + 1 );
				cd_drive	= (int)( szBuffer[0] - 'a' );

			#if(BACKGROUND_BITMAP)

				switch ( idCtl ) {

					//-------------------------------------------------------------------
					// IDD_MOHAVI
					//-------------------------------------------------------------------
					case IDD_PREVIEWS:
					{
						Msg( __LINE__, TEXT(__FILE__), TEXT("IDD_PREVIEWS Selected." ));
						// show the previews in succession.  each will wait for the previous to finish
						// before playing.
						unsigned int success;

						char filepath[MAX_PATH];
						_snprintf(filepath, MAX_PATH, "%s%s", szArgvPath, SC4AVI_FILENAME);

						success = GlobalMainWindow->Run_OpenFile(cd_drive, filepath, true);
//						if (success != 0) {
//							success = GlobalMainWindow->Run_OpenFile(cd_drive, BFAVI_FILENAME, true);
//						}
/*
						if (success == 0) {
							std::wstring wideBuffer = TheGameText->fetch("Autorun:CantRunAVIs");
							std::wstring wideBuffer2 = TheGameText->fetch("Autorun:Error");
							int length = wideBuffer.length();
							WideCharToMultiByte( CodePage, 0, wideBuffer.c_str(), length+1, szBuffer, _MAX_PATH, NULL, NULL );
							length = wideBuffer2.length();
							WideCharToMultiByte( CodePage, 0, wideBuffer2.c_str(), length+1, szBuffer2, _MAX_PATH, NULL, NULL );
							MessageBox( NULL, szBuffer, szBuffer2, MB_APPLMODAL | MB_OK );
						}
*/
					}
					break;

					case IDD_HELP:
					{
						std::wstring wFileName;
						wFileName = Locale_GetString(HELP_FILENAME);
						
						std::string fname;
						const wchar_t *tmp = wFileName.c_str();
						char hack[2] = "a";
						while (*tmp)
						{
							hack[0] = (char)( *tmp & 0xFF );
							fname.append( hack );
							tmp++;
						}

						char newdir[MAX_PATH];
						char olddir[MAX_PATH];
						char filepath[MAX_PATH];

						GetCurrentDirectory(MAX_PATH, olddir);

						_snprintf(newdir, MAX_PATH, "%ssupport", szArgvPath);
						SetCurrentDirectory(newdir);

						_snprintf(filepath, MAX_PATH, "%s%s", szArgvPath, fname.c_str());

						unsigned int success;
						success = GlobalMainWindow->Run_OpenFile(cd_drive, filepath, false);

						SetCurrentDirectory(olddir);

/*
						if (success == 0) {
							std::wstring wideBuffer = TheGameText->fetch("Autorun:CantRunHelp");
							std::wstring wideBuffer2 = TheGameText->fetch("Autorun:Error");
							int length = wideBuffer.length();
							WideCharToMultiByte( CodePage, 0, wideBuffer.c_str(), length+1, szBuffer, _MAX_PATH, NULL, NULL );
							length = wideBuffer2.length();
							WideCharToMultiByte( CodePage, 0, wideBuffer2.c_str(), length+1, szBuffer2, _MAX_PATH, NULL, NULL );
							MessageBox( NULL, szBuffer, szBuffer2, MB_APPLMODAL | MB_OK );
						}
*/
					}
					break;

					//-------------------------------------------------------------------
					// IDD_CANCEL
					//-------------------------------------------------------------------
					case IDD_CANCEL:
						Msg( __LINE__, TEXT(__FILE__), TEXT("IDD_CANCEL Selected." ));
						end_dialog = true;
						break;

					//-------------------------------------------------------------------
					// IDD_OK	-- Install
					// IDD_OK2	-- Play
					//-------------------------------------------------------------------
					case IDD_OK:
					case IDD_OK2:
					case IDD_OK3:
					case IDD_OK4:

//						if( !Is_On_CD( PRODUCT_VOLUME_CD1 ) && IsEnglish ) {
						if( !Is_On_CD( PRODUCT_VOLUME_CD1 )) {

							//-----------------------------------------------------------
							// If false is returned, then CANCEL was pressed.
							//-----------------------------------------------------------
							char volume_to_match[ MAX_PATH ];

							Reformat_Volume_Name( PRODUCT_VOLUME_CD1, volume_to_match );
//							result = Prompt_For_CD( window_handle, volume_to_match, IDS_INSERT_CDROM_WITH_VOLUME1, IDS_EXIT_MESSAGE2, &cd_drive );
							result = Prompt_For_CD( window_handle, volume_to_match, "Autorun:InsertCDROMWithVolume1", "Autorun:ExitMessage2", &cd_drive );
						}

						if ( result ) {
							if ( idCtl == IDD_OK ) {
								Msg( __LINE__, TEXT(__FILE__), TEXT("IDD_OK Selected." ));
								result = GlobalMainWindow->Run_Setup( window_handle, &dlg_rect, cd_drive );
							} else if ( idCtl == IDD_OK2 ) {
								Msg( __LINE__, TEXT(__FILE__), TEXT("IDD_OK2 Selected." ));
								result = GlobalMainWindow->Run_Game( window_handle, &dlg_rect );
							} else if (idCtl == IDD_OK3 ) {
								Msg( __LINE__, TEXT(__FILE__), TEXT("IDD_OK3 Selected, running WorldBuilder." ));
								result = GlobalMainWindow->Run_WorldBuilder( window_handle, &dlg_rect );
							} else if (idCtl == IDD_OK4 ) {
								Msg( __LINE__, TEXT(__FILE__), TEXT("IDD_OK4 Selected, running PatchGet." ));
								result = GlobalMainWindow->Run_PatchGet( window_handle, &dlg_rect );
							}
						}

						if ( result ) {
							end_dialog = true;
						}
						break;

				#if(SHOW_MOH_DEMO)
					//-------------------------------------------------------------------
					// Launch demo from CD.
					//-------------------------------------------------------------------
					case IDD_VIEW_DEMO:

						if( !Is_On_CD( PRODUCT_VOLUME_CD2 )) {

							//-----------------------------------------------------------
							// If false is returned, then CANCEL was pressed.
							//-----------------------------------------------------------
							char volume_to_match[ MAX_PATH ];

							Reformat_Volume_Name( PRODUCT_VOLUME_CD2, volume_to_match );
//							result = Prompt_For_CD( window_handle, volume_to_match, IDS_INSERT_CDROM_WITH_VOLUME2, IDS_EXIT_MESSAGE2, &cd_drive );
							result = Prompt_For_CD( window_handle, volume_to_match, AsciiString("Autorun:InsertCDROMWithVolume2"), AsciiString("Autorun:ExitMessage2"), &cd_drive );
						}

						if ( result ) {
							Msg( __LINE__, TEXT(__FILE__), TEXT("IDD_VIEW_DEMO Selected." ));
							result = GlobalMainWindow->Run_Demo( window_handle, &dlg_rect, cd_drive );
						}

						if ( result ) {
							end_dialog = true;
						}

						break;
				#endif

				#if( SHOW_GAMESPY_BUTTON )
					//-------------------------------------------------------------------
					// Launch GameSpy Website.
					//-------------------------------------------------------------------
					case IDD_GAMESPY:
						Msg( __LINE__, TEXT(__FILE__), TEXT("IDD_GAMESPY Selected." ));
						if( ViewHTML( GAMESPY_WEBSITE )) 
						{
							end_dialog = true;
						} 
						else 
						{
							Error_Message( Main::hInstance, AsciiString("Autorun:Generals"), AsciiString("Autorun:CantFindExplorer"), GAME_WEBSITE );
						}
						break;
				#endif

					//-------------------------------------------------------------------
					// Create a new online account.
					//-------------------------------------------------------------------
					case IDD_NEW_ACCOUNT:
						Msg( __LINE__, TEXT(__FILE__), TEXT("IDD_NEW_ACCOUNT Selected." ));
						result = GlobalMainWindow->Run_New_Account( window_handle, &dlg_rect );
						if ( result ) {
							end_dialog = true;
						}
						break;

					//-------------------------------------------------------------------
					// IDD_REGISTER
					//-------------------------------------------------------------------
					case IDD_REGISTER:
						Msg( __LINE__, TEXT(__FILE__), TEXT("IDD_REGISTER Selected." ));
						result = GlobalMainWindow->Run_Register( window_handle, &dlg_rect );
						if ( result ) {
							end_dialog = true;
						}
						break;

					//-------------------------------------------------------------------
					// IDD_INTERNET
					//-------------------------------------------------------------------
					case IDD_INTERNET:
						Msg( __LINE__, TEXT(__FILE__), TEXT("IDD_INTERNET Selected." ));
						if( ViewHTML( GAME_WEBSITE )) {
							end_dialog = true;
						} 
						else 
						{
							Error_Message( Main::hInstance, "Autorun:Generals", "Autorun:CantFindExplorer", GAME_WEBSITE );
						}
						break;

					//-------------------------------------------------------------------
					// IDD_UPDATE
					//-------------------------------------------------------------------
					case IDD_UPDATE:
						Msg( __LINE__, TEXT(__FILE__), TEXT("IDD_UPDATE Selected." ));
						result = GlobalMainWindow->Run_Auto_Update( window_handle, &dlg_rect );
						if ( result ) {
							end_dialog = true;
						}
						break;

					//-------------------------------------------------------------------
					// IDD_EXPLORE
					//-------------------------------------------------------------------
					case IDD_EXPLORE:
						Msg( __LINE__, TEXT(__FILE__), TEXT("IDD_EXPLORE Selected." ));
						GlobalMainWindow->Run_Explorer( "", window_handle, &dlg_rect );
						end_dialog = true;
						break;

					//-------------------------------------------------------------------
					// IDD_UNINSTALL
					//-------------------------------------------------------------------
					case IDD_UNINSTALL:
						Msg( __LINE__, TEXT(__FILE__), TEXT("IDD_UNINSTALL Selected." ));
						result = GlobalMainWindow->Run_Uninstall( window_handle, &dlg_rect );

						//---------------------------------------------------------------
						// MML 5/27/99:  I am exiting here because the we launch 
						// Uninstll.exe which in turn launches Uninst.exe thus 
						// ::Run_Install ends before Uninst.exe is done.
						//---------------------------------------------------------------
#if 1
						if ( result ) {
							end_dialog = true;
						}
#endif
						break;

					default:
						break;
				}

				//-----------------------------------------------------------------------
				// Exit Autorun.
				//-----------------------------------------------------------------------
				if( end_dialog ) {

					for ( i = 0; i < NUM_BUTTONS; i++ ) {
						if ( ButtonList[i] ) {
							delete( ButtonList[i] );
							ButtonList[i] = NULL;
						}
					}
					if ( hpal ) {
						DeleteObject( hpal );
					}
					if ( hBitmap ) {
						DeleteObject( hBitmap );
					}
					for( i = 0; i < NUM_FLICKER_FRAMES; i++ ) {
						DeleteObject( hFlicker[i] );
						hFlicker[i] = 0;
					}
					Stop_Sound_Playing();
					KillTimer( window_handle, timer_id );
					EndDialog( window_handle, idCtl );
				}

			#else
				if ( hStaticBrush ) {
					DeleteObject( hStaticBrush );
					hStaticBrush = 0;
				}
				EndDialog( window_handle, idCtl );
				KillTimer( window_handle, timer_id );
				KillTimer( window_handle, gem_timer_id );
			#endif

			}
			break;

		//-------------------------------------------------------------------------------
		// This message is the response to the Close Button in upper right corner.
		//-------------------------------------------------------------------------------
		case WM_SYSCOMMAND:

			if ( w_param == SC_CLOSE ) {
				#if(BACKGROUND_BITMAP)

					for ( i = 0; i < NUM_BUTTONS; i++ ) {
						if ( ButtonList[i] ) {
							delete( ButtonList[i] );
							ButtonList[i] = NULL;
						}
					}
					if ( hpal ) {
						DeleteObject( hpal );
					}
					if ( hBitmap ) {
						DeleteObject( hBitmap );
					}
					for( i = 0; i < NUM_FLICKER_FRAMES; i++ ) {
						DeleteObject( hFlicker[i] );
						hFlicker[i] = 0;
					}

				#else
					if ( hStaticBrush ) {
						DeleteObject( hStaticBrush );
						hStaticBrush = 0;
					}
				#endif

				//-----------------------------------------------------------------------
				// Stop the sound if still going.
				//-----------------------------------------------------------------------
				Stop_Sound_Playing();

				//-----------------------------------------------------------------------
				// Delete the arguments.
				//-----------------------------------------------------------------------
				if ( Args ) {
					delete( Args );
					Args = NULL;
				}
				KillTimer( window_handle, timer_id );
				EndDialog( window_handle, w_param );
			}
			break;

		//-------------------------------------------------------------------------------
		// WM_SYSCOLORCHANGE Message.
		// If your applications uses controls in Windows 95/NT, forward the 
		// WM_SYSCOLORCHANGE message to the controls. 
		//-------------------------------------------------------------------------------
		#if( !BACKGROUND_BITMAP )
		case WM_SYSCOLORCHANGE:
			if ( hStaticBrush ) {
				DeleteObject( hStaticBrush );
				hStaticBrush = CreateSolidBrush( GetSysColor( COLOR_WINDOW ));
			}
			break;
		#endif

		//-------------------------------------------------------------------------------
		//	WM_CTLCOLOR Message.
		//	wParam					Handle to Child Window's device context
		//	LOWORD( lParam )		Child Window handle
		//	HIWORD( lParam )		Type of Window: 	CTLCOLOR_MSGBOX, _EDIT, _LISTBOX, _BTN, _DLG, _SCROLLBAR, _STATIC
		//
		//	WM_CTLCOLORMSGBOX
		//	WM_CTLCOLOREDIT
		//	WM_CTLCOLORLISTBOX
		//	WM_CTLCOLORBTN
		//	WM_CTLCOLORDLG
		//	WM_CTLCOLORSCROLLBAR 
		//	WM_CTLCOLORSTATIC
		//	#define WM_CTLCOLOR								0x0019
		//	#define GET_WM_CTLCOLOR_HDC (wp, lp, msg)		(HDC)(wp)
		//	#define GET_WM_CTLCOLOR_HWND(wp, lp, msg)		(HWND)(lp)
		//	#define GET_WM_CTLCOLOR_TYPE(wp, lp, msg)		(WORD)(msg - WM_CTLCOLORMSGBOX)
		//	#define GET_WM_CTLCOLOR_MSG (type)				(WORD)(WM_CTLCOLORMSGBOX+(type))
		//-------------------------------------------------------------------------------
		#if( !BACKGROUND_BITMAP )
		case WM_CTLCOLOR:
			if ( HIWORD( l_param ) == CTLCOLOR_STATIC ) {

				SetTextColor(( HDC )w_param, GetSysColor( COLOR_WINDOWTEXT ));
				SetBkColor( (HDC)wParam, GetSysColor( COLOR_WINDOW ));
//				SetBkColor(( HDC )w_param, RGB( 192, 192, 192 ));

				UnrealizeObject( hStaticBrush );									// reset the origin of the brush next time used.
				point.x = point.y = 0;												// create a point.
				ClientToScreen( window_handle, &point );						// translate into screen coordinates.
				SetBrushOrgEx( (HDC)w_param, point.x, point.y, NULL );	// New Origin to use when next selected.
				return((LRESULT) hStaticBrush );
			}
		#endif

		//===============================================================================
		// Check where Left Mouse button was pressed.
		//===============================================================================
		#if(BACKGROUND_BITMAP)
		case WM_LBUTTONDOWN:
			{
				RECT rect;

				//----------------------------------------------------------------------
				// Get mouse coordinates.
				//----------------------------------------------------------------------
				mouse_x = LOWORD( l_param );
				mouse_y = HIWORD( l_param );

				//----------------------------------------------------------------------
				// For each button in the list...
				//----------------------------------------------------------------------
				for ( i = 0; i < NUM_BUTTONS; i++ ) {

					//-------------------------------------------------------------------
					// If mouse was clicked in one of the "buttons", then change
					// that button's state to "pressed".
					//-------------------------------------------------------------------
					if ( ButtonList[i] && ButtonList[i]->Is_Mouse_In_Region( mouse_x, mouse_y )) {

						if ( ButtonList[i]->Get_State() != DrawButton::PRESSED_STATE ) {

							ButtonList[i]->Return_Area ( &rect );
							ButtonList[i]->Set_State( DrawButton::PRESSED_STATE );
							InvalidateRect( window_handle, &rect, FALSE );

							Msg( __LINE__, TEXT(__FILE__), TEXT("WM_LBUTTONDOWN -- %s. rect = [%d,%d,%d,%d]."), 
								ButtonList[i]->Return_Normal_Bitmap(), rect.left, rect.top, rect.right, rect.bottom );

							UpdateWindow( window_handle );
						}
						break;
					} 
				}
			}
			break;
		#endif

		//===============================================================================
		// Check where Left Mouse button was released.
		//===============================================================================
		#if(BACKGROUND_BITMAP)
		case WM_LBUTTONUP:
			{
				RECT rect;
				int focus_index = 0;
				int found_focus = -1;

				//-----------------------------------------------------------------------
				// Get mouse coordinates.
				//-----------------------------------------------------------------------
				mouse_x = LOWORD( l_param );
				mouse_y = HIWORD( l_param );

				//=======================================================================
				// focus_index = previous PRESSED/FOCUSED button.
				// found_focus = new PRESSED/FOCUSED button ( if different ).
				//=======================================================================

				//-----------------------------------------------------------------------
				// First find the button that is either focused or pressed.
				//-----------------------------------------------------------------------
				for ( i = 0; i < NUM_BUTTONS; i++ ) {
					if ( ButtonList[i] ) {

						//---------------------------------------------------------------
						// Save index of button with focus.
						//---------------------------------------------------------------
						if(	ButtonList[i]->Get_State() == DrawButton::FOCUS_STATE || 
						  	ButtonList[i]->Get_State() == DrawButton::PRESSED_STATE ) {
							focus_index = i;
						}
					}
				}

				//-----------------------------------------------------------------------
				// Then find the button that is to be focused or pressed.
				//-----------------------------------------------------------------------
				for ( i = 0; i < NUM_BUTTONS; i++ ) {
					if ( ButtonList[i] && ButtonList[i]->Is_Mouse_In_Region( mouse_x, mouse_y )) {
						found_focus = i;
					}
				}

				//-----------------------------------------------------------------------
				// If new button is not found... 
				//-----------------------------------------------------------------------
				if ( found_focus == -1 ) {

					//-------------------------------------------------------------------
					// Make sure previously focused/pressed button is now is a 
					// focused state and no action is taken.  This occurs when 
					// mouse is clicked outside of any button areas.
					//-------------------------------------------------------------------
					if ( ButtonList[focus_index] && ( ButtonList[focus_index]->Get_State() != DrawButton::FOCUS_STATE )) {

						ButtonList[focus_index]->Set_State( DrawButton::FOCUS_STATE );
						ButtonList[focus_index]->Return_Area ( &rect );
						InvalidateRect( window_handle, &rect, FALSE );

						Msg( __LINE__, TEXT(__FILE__), TEXT("WM_LBUTTONUP -- %s[FOCUS_STATE] = [x=%d, y=%d, w=%d, h=%d]."), 
							ButtonList[ focus_index ]->Return_Normal_Bitmap(),	rect.left, rect.top, rect.right, rect.bottom );

						UpdateWindow( window_handle );
					}

				} else {

					//-------------------------------------------------------------------
					// Buttons are one and the same.
					//-------------------------------------------------------------------
					if( focus_index == found_focus ) {

						ButtonList[ found_focus ]->Set_State( DrawButton::FOCUS_STATE );
						ButtonList[ found_focus ]->Return_Area ( &rect );
						InvalidateRect( window_handle, &rect, FALSE );

						Msg( __LINE__, TEXT(__FILE__), TEXT("WM_LBUTTONUP -- %s[FOCUS_STATE] = [x=%d, y=%d, w=%d, h=%d]."), 
							ButtonList[ found_focus ]->Return_Normal_Bitmap(), rect.left, rect.top, rect.right, rect.bottom );

						UpdateWindow( window_handle );

					} else {

						//---------------------------------------------------------------
						// Make previously focused button, Normal...
						//---------------------------------------------------------------
						if ( ButtonList[ focus_index ] ) {
					
							ButtonList[ focus_index ]->Set_State( DrawButton::NORMAL_STATE );
							ButtonList[ focus_index ]->Return_Area ( &rect );
							InvalidateRect( window_handle, &rect, FALSE );
		
							Msg( __LINE__, TEXT(__FILE__), TEXT("WM_LBUTTONUP -- %s[NORMAL_STATE] = [x=%d, y=%d, w=%d, h=%d]."), 
								ButtonList[ focus_index ]->Return_Normal_Bitmap(),
								rect.left, rect.top, rect.right, rect.bottom );
						
							UpdateWindow( window_handle );
						}

						//---------------------------------------------------------------
						// ...and the new button now has focus.
						//---------------------------------------------------------------
						if ( ButtonList[ found_focus ] ) {
					
							ButtonList[ found_focus ]->Set_State( DrawButton::FOCUS_STATE );
							ButtonList[ found_focus ]->Return_Area ( &rect );
							InvalidateRect( window_handle, &rect, FALSE );

							Msg( __LINE__, TEXT(__FILE__), TEXT("WM_LBUTTONUP -- %s[FOCUS_STATE] = [x=%d, y=%d, w=%d, h=%d]."), 
								ButtonList[ found_focus ]->Return_Normal_Bitmap(), rect.left, rect.top, rect.right, rect.bottom );

							UpdateWindow( window_handle );
						}
					}
				}

				//-----------------------------------------------------------------------
				// Repaint the Window now.
				//-----------------------------------------------------------------------
				nResult = UpdateWindow( window_handle );

				//-----------------------------------------------------------------------
				// Do the focus button's action.
				//-----------------------------------------------------------------------
				if ( found_focus >= 0 ) {
					if (( ButtonList[found_focus] ) && 
						( ButtonList[found_focus]->Get_State() == DrawButton::FOCUS_STATE ) && 
						( ButtonList[found_focus]->Is_Mouse_In_Region( mouse_x, mouse_y ))) {
							SendMessage( window_handle, WM_COMMAND, ButtonList[found_focus]->Return_Id(), 0L );
							break;
					}
				}
			}
			break;
		#endif

		//-------------------------------------------------------------------------------
		// Check Mouse moves over buttons.
		//-------------------------------------------------------------------------------
//#if(DISABLE_KEYBOARD)
		#if(BACKGROUND_BITMAP)
		case WM_MOUSEMOVE:
			{
				RECT rect;
				int j;
				int done = 0;

				//-----------------------------------------------------------------------
				// Get mouse coordinates.
				//-----------------------------------------------------------------------
				mouse_x = LOWORD( l_param );
				mouse_y = HIWORD( l_param );

			#if(USE_MOUSE_MOVES)
				//-----------------------------------------------------------------------
				// Reset most current button.									         
				//-----------------------------------------------------------------------
				CurrentButton = 0;
			#endif

				//-----------------------------------------------------------------------
				// For each button in the list...								         
				//-----------------------------------------------------------------------
				i = 0;
				while( i < NUM_BUTTONS && !done ) {

					//-------------------------------------------------------------------
					// For each button, check if mouse is in it's area.			        
					//-------------------------------------------------------------------
					if ( ButtonList[i] && ButtonList[i]->Is_Mouse_In_Region( mouse_x, mouse_y )) {

						//---------------------------------------------------------------
						// This is now the current button.							       
						//---------------------------------------------------------------
						CurrentButton = ButtonList[i]->Return_Id();

						if( CurrentButton != LastButton ) {

							//-----------------------------------------------------------
							// Make all other buttons, NORMAL.
							//-----------------------------------------------------------
							for ( j = 0; j < NUM_BUTTONS; j++ ) {
								if ( ButtonList[j] ) {
									ButtonList[j]->Set_State( DrawButton::NORMAL_STATE );
//									Msg( __LINE__, TEXT(__FILE__), TEXT("WM_MOUSEMOVE -- %s[NORMAL_STATE]]."), ButtonList[j]->Return_Normal_Bitmap());
								}
							}

							if ( w_param & MK_LBUTTON ) {

								//--------------------------------------------------------
								// Left Mouse button is pressed! Make it a pressed button!
								//--------------------------------------------------------
								if ( ButtonList[i] && ButtonList[i]->Get_State() != DrawButton::PRESSED_STATE ) {
									ButtonList[i]->Set_State( DrawButton::PRESSED_STATE );
//									Msg( __LINE__, TEXT(__FILE__), TEXT("WM_MOUSEMOVE -- %s[PRESSED_STATE]."), ButtonList[i]->Return_Normal_Bitmap());
								}

							} else {
		
								//--------------------------------------------------------
								// If this button is not already focused, give it the focus. 
								//--------------------------------------------------------
								if ( ButtonList[i] && ButtonList[i]->Get_State() != DrawButton::FOCUS_STATE ) {
									ButtonList[i]->Set_State( DrawButton::FOCUS_STATE );
//									Msg( __LINE__, TEXT(__FILE__), TEXT("WM_MOUSEMOVE -- %s[FOCUS_STATE]."), ButtonList[i]->Return_Normal_Bitmap());
								}
							}	// end of if

							//-----------------------------------------------------------
							// Get the area of the button, and post it for updating.
							//-----------------------------------------------------------
							for ( j = 0; j < NUM_BUTTONS; j++ ) {
								if ( ButtonList[j] ) {
									ButtonList[j]->Return_Area ( &rect );
									InvalidateRect( window_handle, &rect, FALSE );
								}
							}

							//-----------------------------------------------------------
							// Repaint now!
							//-----------------------------------------------------------
							UpdateWindow( window_handle );

							done = 1;
						}

					}	// end of if
					i++;

				}	// end of for


			#if( USE_MOUSE_MOVES )
	        	//-----------------------------------------------------------------------
				// If a MouseMove was found to be in one of the buttons, then 
				// CurrentButton will have a value.
        		//-----------------------------------------------------------------------
				if ( CurrentButton != 0 ) {

					LastButton = CurrentButton;

		        	//-------------------------------------------------------------------
					// If we are still in the same button, don't make a sound!
				  	//-------------------------------------------------------------------
					if ( LastButton != PrevButton ) {
						PrevButton = LastButton;
						PlaySound( szButtonWav, Main::hModule, SND_ASYNC | SND_RESOURCE );
					}
				}
			#endif
			}
			break;

		#endif	// Background_Bitmap flag
//#endif

		//-------------------------------------------------------------------------------
		// Repaint when focus is restored (does partial repaint), and when
		// mouse is double clicked on dialog ( full repaint ).
		//-------------------------------------------------------------------------------
		case WM_LBUTTONDBLCLK:
		case WM_SETFOCUS:
			InvalidateRect( window_handle, &dlg_rect, TRUE );
//			nResult = UpdateWindow( window_handle );
//			Msg( __LINE__, TEXT(__FILE__), TEXT("WM_LBUTTONDBLCLK -- dlg_rect = [x=%d, y=%d, w=%d, h=%d]."), 
//				dlg_rect.left, dlg_rect.top, dlg_rect.right, dlg_rect.bottom );
			break;

		#if(BACKGROUND_BITMAP)
		//-------------------------------------------------------------------------------
		// bit 30 of lParam - Specifies the previous key state. 
		// The value is 1 if the key is down before the message is sent, 
		// or it is 0 if the key is up.
		//-------------------------------------------------------------------------------
		case WM_KEYUP:
			{
//				int j = 0;

				switch( w_param ) {

					case VK_ESCAPE:
						SendMessage( window_handle, WM_SYSCOMMAND, SC_CLOSE, 0L );
						break;

//#if(DISABLE_KEYBOARD)
					case VK_RETURN:
						//---------------------------------------------------------------
						// If the Return/Enter key is pressed... find the focused
						//	button and call its action.
						//---------------------------------------------------------------
//						result = ( l_param & 0x40000000 );
						for ( i = 0; i < NUM_BUTTONS; i++ ) {
							if ( ButtonList[i] && ButtonList[i]->Get_State() == DrawButton::FOCUS_STATE ) {
								SendMessage( window_handle, WM_COMMAND, ButtonList[i]->Return_Id(), 0L );
								break;
							}
						}
						break;
//#endif

//#if(DISABLE_KEYBOARD)
					case VK_TAB:
					case VK_DOWN:
						{
							//-----------------------------------------------------------
							// Find the button with focus and "tab" to the next button by finding 
							// the next valid index.  If past last button, circle back to the top.
							//-----------------------------------------------------------
							int focused_button = 0;
							int next_button = 0;

							for ( i = 0; i < NUM_BUTTONS; i++ ) {
								if ( ButtonList[i] && ButtonList[i]->Get_State() == DrawButton::FOCUS_STATE ) {

									focused_button = i;
									next_button = i+1;

									if ( next_button >= NUM_BUTTONS ) {
										next_button = 0;
									}
									while (( next_button < NUM_BUTTONS ) && !ButtonList[ next_button ] ) {
										next_button++;
									}

									if ( next_button >= NUM_BUTTONS ) {
										next_button = 0;
										while (( next_button < NUM_BUTTONS ) && !ButtonList[ next_button ] ) {
											next_button++;
										}
									}
									break;
								}
							}

							//-----------------------------------------------------------
							// Set the previous button to Normal status.
							//-----------------------------------------------------------
							if ( ButtonList[focused_button] && ( ButtonList[focused_button]->Get_State() != DrawButton::NORMAL_STATE )) {

								ButtonList[focused_button]->Set_State( DrawButton::NORMAL_STATE );
								ButtonList[focused_button]->Return_Area ( &rect );
								InvalidateRect( window_handle, &rect, FALSE );

								Msg( __LINE__, TEXT(__FILE__), TEXT("VK_DOWN/VK_TAB -- %s = [%s]."), ButtonList[focused_button]->Return_Normal_Bitmap(), "NORMAL_STATE" );
							}

							//-----------------------------------------------------------
							// Set the new button to focus status.
							//-----------------------------------------------------------
							if ( ButtonList[next_button] && ( ButtonList[next_button]->Get_State() != DrawButton::FOCUS_STATE )) {

								ButtonList[next_button]->Set_State( DrawButton::FOCUS_STATE );
								ButtonList[next_button]->Return_Area ( &rect );
								InvalidateRect( window_handle, &rect, FALSE );
								PlaySound( szButtonWav, Main::hModule, SND_ASYNC | SND_RESOURCE );

								Msg( __LINE__, TEXT(__FILE__), TEXT("VK_DOWN/VK_TAB -- %s = [%s]."), ButtonList[next_button]->Return_Normal_Bitmap(), "FOCUS_STATE" );
							}
						}
						break;

					case VK_UP:
						{
							//-----------------------------------------------------------
							// Find the button with focus and "tab" to the next button by finding 
							// the next valid index.  If past last button, circle back to the top.
							//-----------------------------------------------------------
							int focused_button = 0;
							int next_button = 0;

							for ( i = 0; i < NUM_BUTTONS; i++ ) {
								if ( ButtonList[i] && ButtonList[i]->Get_State() == DrawButton::FOCUS_STATE ) {

									focused_button = i;
									next_button	= i-1;

									if ( next_button < 0 ) {
										next_button = NUM_BUTTONS - 1;
									}
									while (( next_button > 0 ) && !ButtonList[ next_button ] ) {
										next_button--;
									}

									if ( !ButtonList[ next_button ]) {
										next_button = NUM_BUTTONS - 1;
									}
									while (( next_button >= 0 ) && !ButtonList[ next_button ] ) {
										next_button--;
									}
									break;
								}
							}

							//-----------------------------------------------------------
							// Set the previous button to Normal status.
							//-----------------------------------------------------------
							if ( ButtonList[focused_button] && ( ButtonList[focused_button]->Get_State() != DrawButton::NORMAL_STATE )) {

								ButtonList[focused_button]->Set_State( DrawButton::NORMAL_STATE );
								ButtonList[focused_button]->Return_Area ( &rect );
								InvalidateRect( window_handle, &rect, FALSE );

								Msg( __LINE__, TEXT(__FILE__), TEXT("VK_DOWN/VK_TAB -- %s = [%s]."), ButtonList[focused_button]->Return_Normal_Bitmap(), "NORMAL_STATE" );
							}

							//-----------------------------------------------------------
							// Set the new button to focus status.
							//-----------------------------------------------------------
							if ( ButtonList[next_button] && ( ButtonList[next_button]->Get_State() != DrawButton::FOCUS_STATE )) {

								ButtonList[next_button]->Set_State( DrawButton::FOCUS_STATE );
								ButtonList[next_button]->Return_Area ( &rect );
								InvalidateRect( window_handle, &rect, FALSE );
								PlaySound( szButtonWav, Main::hModule, SND_ASYNC | SND_RESOURCE );

								Msg( __LINE__, TEXT(__FILE__), TEXT("VK_DOWN/VK_TAB -- %s = [%s]."), ButtonList[next_button]->Return_Normal_Bitmap(), "FOCUS_STATE" );
							}
						}
						break;
//#endif

				}	/* end of switch */
			}	/* end of case stmt */

			return ( 0 );

		#endif
	}
	return( FALSE );
}

//*****************************************************************************
// STOP_SOUND_PLAYING -- Stop the background sound.
//                                                                 
// INPUT:		none.
//                                                                 
// OUTPUT:		none.
//                                                                 
// WARNINGS:	Will stop any sound started by PlaySound.
//                                                                 
// HISTORY:                                                                
//   06/04/1999  MML : Created.                                            
//=============================================================================

void Stop_Sound_Playing ( void )
{
	PlaySound( NULL, NULL, SND_ASYNC | SND_FILENAME );
}

//*****************************************************************************
// OPTIONS -- Find any user options and set the correct flags      
//                                                                 
// INPUT:   int argc      =  no. of arguments to check        		 
//          BYTE *argv[]  =  ptr to actual command line parameters 
//                                                                 
// OUTPUT:                                                         
//                                                                 
// WARNINGS:                                                       
//                                                                 
// HISTORY:                                                                
//   06/04/1996  MML : Created.                                            
//=============================================================================

BOOL Options( Command_Line_Arguments *Orgs )
{
	int		i;
	BOOL	result = TRUE;
	char 	buffer[ MAX_ARGUMENT_LENGTH ];

	//-------------------------------------------------------------------------
	// Scan arguments for any options ( / or - followed by a letter)
	//-------------------------------------------------------------------------
	for ( i = 0; i < Orgs->Get_argc(); i++ ) {

		//---------------------------------------------------------------------
		// Get the next item in the list.
		//---------------------------------------------------------------------
		memset( buffer, '\0', sizeof( buffer ));
		strcpy( buffer, Orgs->Get_argv(i));

		Msg( __LINE__, TEXT(__FILE__), TEXT("Options -- Argument[%d] = %s."), i, buffer );

		//---------------------------------------------------------------------
		// If starts with / or -
		//---------------------------------------------------------------------
		if (( buffer[0]  == '/' ) || ( buffer[0]  == '-' ))	{

			switch ( tolower( buffer[1] )) {

				//-------------------------------------------------------------
				// Retrieve the game's version info.
				//-------------------------------------------------------------
				case 'v':
					{
						char szPath   [ MAX_PATH ];
						char szVersion[ MAX_PATH ];

						Make_Current_Path_To( SETUP_INI_FILE1, szPath );
						GetPrivateProfileString( "Setup", "Version", "1.0", szVersion, sizeof( szVersion ), szPath );

						LoadString( Main::hInstance, IDS_VERSION_STRING, szBuffer,  MAX_PATH );

//						sprintf( szBuffer3, "V %s", szVersion );
						sprintf( szBuffer3, szBuffer, szVersion );
//						strcpy( szBuffer, szRegistryKey );

						MessageBox( NULL, szBuffer3, "Autorun", MB_TASKMODAL | MB_OK );
						result = FALSE;
					}
					break;

				//-------------------------------------------------------------
				// Bypass the volume check.
				//-------------------------------------------------------------
				case 'n':
					{
						HANDLE  handle;
						WIN32_FIND_DATA FindFileData;

						memset( szVolumeName, '\0', MAX_PATH );

						//-----------------------------------------------------
						// If we think we are on CD2, then use PRODUCT_VOLUME_CD2.
						//-----------------------------------------------------
						Make_Current_Path_To( MOH_DEMO_PROGRAM, szBuffer );
						handle = FindFirstFile( szBuffer, &FindFileData );
						if ( handle == INVALID_HANDLE_VALUE ) {
							strcpy( szVolumeName, PRODUCT_VOLUME_CD1 );
						} else {
							strcpy( szVolumeName, PRODUCT_VOLUME_CD2 );
							FindClose( handle );	
						}

						strcpy( Arguments[ NumberArguments++ ], buffer );
					}
					break;

			#if( _DEBUG )

				case 'c':
					if( buffer[2] == 'd' ) {
						szCDDrive[0] = buffer[3];
						szCDDrive[1] = ':';						
						szCDDrive[2] = '\\';
					}
					break;

				//-------------------------------------------------------------
				// Change languages?
				//-------------------------------------------------------------
				case 'l':
					{
						//-----------------------------------------------------
						//	Identifier		Meaning 
						//	932				Japan 
						//	936				Chinese (PRC, Singapore) 
						//	949				Korean 
						//	1252			Windows 3.1 Latin 1 (US, Western Europe) 
						//-----------------------------------------------------
						char *temp = buffer+2;
						int number = atoi( temp );

						switch( number ) {

								case LANG_GER:
									LanguageToUse = LANG_GER;
									CodePage = 1252;
									break;

								case LANG_FRE:
									LanguageToUse = LANG_FRE;
									CodePage = 1252;
									break;

								case LANG_JAP:
									LanguageToUse = LANG_JAP;
									CodePage = 932;
									break;

								case LANG_KOR:
									LanguageToUse = LANG_KOR;
									CodePage = 949;
									break;

								case LANG_CHI:
									LanguageToUse = LANG_CHI;
									CodePage = 950;
									break;

								case LANG_USA:
								default:
									LanguageToUse = LANG_USA;
									CodePage = 1252;
									break;
						}
					}
					break;


			#endif

				default:
					strcpy( Arguments[ NumberArguments++ ], buffer );
					break;
			}
		}
	}

	Msg( __LINE__, TEXT(__FILE__), TEXT("Options -- Language = %d"), Language );
	Msg( __LINE__, TEXT(__FILE__), TEXT("Options -- CodePage = %d"), CodePage );

#if(0)
	struct lconv *info = localeconv();
	char szDefaultLangID[ MAX_PATH ];

	GetLocaleInfo(
		  LOCALE_USER_DEFAULT,	// locale identifier
		  LOCALE_ILANGUAGE,		// type of information
		  szBuffer1,	 		// address of buffer for information
		  MAX_PATH );	 		// size of buffer

	Msg( __LINE__, TEXT(__FILE__), TEXT("Options -- GetLocalInfo = %s"), szBuffer1 );

	sprintf( szDefaultLangID, "%04X", GetUserDefaultLangID());
	Msg( __LINE__, __FILE__, "Options -- GetUserDefaultLangID() = %s", szDefaultLangID );

	sprintf( szDefaultLangID, "%04X", GetSystemDefaultLangID());
	Msg( __LINE__, __FILE__, "Options -- GetSystemDefaultLangID() = %s", szDefaultLangID );
	Msg( __LINE__, __FILE__, "-------------------------------------------------------------" );
#endif

	return( result );
}

//*****************************************************************************
// Valid_Environment -- Make sure this program is run from CD-ROM disc only 
//					 	AND it is a Win95 system.
//
// INPUT:  		none.
//
//	OUTPUT: 	none.
//
// WARNINGS:	returns 0 if ok to continue.
//
// HISTORY:                                                                
//   06/04/1996  MML : Created.                                            
//=============================================================================

BOOL Valid_Environment ( void )
{
	bool result = 0;

	//--------------------------------------------------------------------------
	// Check Windows Version.
	//--------------------------------------------------------------------------

	int length = 0;
	result = WinVersion.Meets_Minimum_Version_Requirements();
  if ( !result ) 
	{
		std::wstring wideBuffer = TheGameText->fetch("GUI:WindowsVersionText");
		std::wstring wideBuffer2 = TheGameText->fetch("GUI:WindowsVersionTitle");
		length = wideBuffer.length();
		WideCharToMultiByte( CodePage, 0, wideBuffer.c_str(), length+1, szBuffer, _MAX_PATH, NULL, NULL );
		length = wideBuffer2.length();
		WideCharToMultiByte( CodePage, 0, wideBuffer2.c_str(), length+1, szBuffer2, _MAX_PATH, NULL, NULL );
		MessageBox( NULL, szBuffer, szBuffer2, MB_APPLMODAL | MB_OK );
	}

	return( result );
}

//****************************************************************************
// LOADRESOURCEBITMAP -- Find & Load the bitmap from the resource.
//                                                                 
// INPUT:   HINSTANCE hInstance -- Program's hInstance.
//          LPTSTR lpString -- name of bitmap to find.
//          HPALETTE FAR *lphPalette -- we will return palette in this.
//
// OUTPUT:  HBITMAP -- handle to the bitmap if found.
//                                                                 
// WARNINGS:
//                                                                 
// HISTORY: Found this routine on MS Developmemt CD, July 1996.
// 	09/26/1996  MML : Created.                                            
//=============================================================================

HBITMAP LoadResourceBitmap( HINSTANCE hInstance, LPTSTR lpString, HPALETTE FAR *lphPalette, bool loading_a_button )
{
//	HDC 		hdc;
	int 		iNumColors;
	HRSRC 		hRsrc;
	HGLOBAL 	hGlobal;
	HBITMAP 	hBitmapFinal = NULL;
	LPBITMAPINFOHEADER lpbi;

	hBitmapFinal = LoadBitmap( hInstance, lpString );

	//--------------------------------------------------------------------------
	// Find the Bitmap in this program's resources.
	//--------------------------------------------------------------------------
	hRsrc = FindResource( hInstance, lpString, RT_BITMAP );
	if ( hRsrc ) {

		//-----------------------------------------------------------------------
		// Load the resource, lock the memory, grab a DC.
		//-----------------------------------------------------------------------
		hGlobal	= LoadResource( hInstance, hRsrc );
		lpbi  	= (LPBITMAPINFOHEADER) LockResource( hGlobal );

		if ( loading_a_button ) {

			//--------------------------------------------------------------------------
			// Set number of colors ( 2 to the nth ).
			//--------------------------------------------------------------------------
			if ( lpbi->biBitCount <= 8 ) {
				iNumColors = ( 1 << lpbi->biBitCount );
			} else {
				iNumColors = 0;
			}

		} else {

			//--------------------------------------------------------------------
			// Get the palette from the resource.  
			//--------------------------------------------------------------------
			*lphPalette = CreateDIBPalette((LPBITMAPINFO) lpbi, &iNumColors );
		}

		//-----------------------------------------------------------------------
		// Free DS and memory used.
		//-----------------------------------------------------------------------
		UnlockResource( hGlobal );
		FreeResource( hGlobal );
	}

	return( hBitmapFinal );
}
 
//*****************************************************************************
// CREATEDIBPALETTE -- Creates the palette from the Bitmap found above.
//                                                                 
// INPUT:   LPBITMAPINFO lpbmi -- bitmap info from header.
//          LPINT lpiNumColors -- number of colors.
//
// OUTPUT:  HPALETTE -- handle to the bitmap if found.
//                                                                 
// WARNINGS:                                                       
//                                                                 
// HISTORY: Found this routine on MS Developmemt CD, July 1996.
// 	09/26/1996  MML : Created.                                            
//=============================================================================
HPALETTE CreateDIBPalette ( LPBITMAPINFO lpbmi, LPINT lpiNumColors )
{
	LPBITMAPINFOHEADER lpbi;
	LPLOGPALETTE lpPal;
	HANDLE hLogPal;
	HPALETTE hPal = NULL;
	int i;

	lpbi = (LPBITMAPINFOHEADER) lpbmi;

	//--------------------------------------------------------------------------
	// Set number of colors ( 2 to the nth ).
	//--------------------------------------------------------------------------
	if ( lpbi->biBitCount <= 8 ) {
		*lpiNumColors = ( 1 << lpbi->biBitCount );
	} else {
		*lpiNumColors = 0;
	}

	//--------------------------------------------------------------------------
	// If bitmap has a palette ( bitcount ), lock some memory and create
	// a palette from the bitmapinfoheader passed in.
	//--------------------------------------------------------------------------
	if ( *lpiNumColors ) {

		hLogPal = GlobalAlloc( GHND, sizeof( LOGPALETTE ) + sizeof( PALETTEENTRY ) * ( *lpiNumColors ));
		lpPal	= (LPLOGPALETTE) GlobalLock( hLogPal );
		lpPal->palVersion	= 0x300;
		lpPal->palNumEntries = (WORD)*lpiNumColors;

		for ( i = 0; i < *lpiNumColors; i++ ) {
			lpPal->palPalEntry[i].peRed   = lpbmi->bmiColors[i].rgbRed;
			lpPal->palPalEntry[i].peGreen = lpbmi->bmiColors[i].rgbGreen;
			lpPal->palPalEntry[i].peBlue  = lpbmi->bmiColors[i].rgbBlue;
			lpPal->palPalEntry[i].peFlags = 0;
		}
		hPal = CreatePalette( lpPal );
		GlobalUnlock( hLogPal );
		GlobalFree( hLogPal );

#if(0)
		StandardFileClass fileout;
		char buff[2];

		fileout.Open( "test.pal", MODE_WRITE_TRUNCATE );
		for ( i = 0; i < *lpiNumColors; i++ ) {
			sprintf( buff, "%d", lpPal->palPalEntry[i].peRed >> 2 );
			fileout.Write(( void *)buff, 2 );
			sprintf( buff, "%d", lpPal->palPalEntry[i].peGreen >> 2 );
			fileout.Write(( void *)buff, 2 );
			sprintf( buff, "%d", lpPal->palPalEntry[i].peBlue >> 2 );
			fileout.Write(( void *)buff, 2 );
		}
		fileout.Close();
#endif

	}
	return( hPal );	
}

//*****************************************************************************
// LOADRESOURCEBUTTON -- Find & Load the bitmap from the resource.
//                                                                 
// INPUT:   HINSTANCE hInstance -- Program's hInstance.
//          LPTSTR lpString -- name of bitmap to find.
//          HPALETTE FAR *lphPalette -- we will return palette in this.
//
// OUTPUT:  HBITMAP -- handle to the bitmap if found.
//                                                                 
// WARNINGS:
//                                                                 
// HISTORY: Found this routine on MS Developmemt CD, July 1996.
// 	09/26/1996  MML : Created.                                            
//=============================================================================
HBITMAP LoadResourceButton( HINSTANCE hInstance, LPTSTR lpString, HPALETTE FAR lphPalette )
{
	HDC 		hdc;
	int 		iNumColors;
	HRSRC 	hRsrc;
	HGLOBAL 	hGlobal;
	HBITMAP 	hBitmapFinal = NULL;
	LPBITMAPINFOHEADER lpbi;

	//--------------------------------------------------------------------------
	// Find the Bitmap in this program's resources.
	//--------------------------------------------------------------------------
	hRsrc = FindResource( hInstance, lpString, RT_BITMAP );
	if ( hRsrc ) {

		//-----------------------------------------------------------------------
		// Load the resource, lock the memory, grab a DC.
		//-----------------------------------------------------------------------
		hGlobal	= LoadResource( hInstance, hRsrc );
		lpbi		= (LPBITMAPINFOHEADER) LockResource( hGlobal );
		hdc		= GetDC( NULL );

		//--------------------------------------------------------------------------
		// Set number of colors ( 2 to the nth ).
		//--------------------------------------------------------------------------
		if ( lpbi->biBitCount <= 8 ) {
			iNumColors = ( 1 << lpbi->biBitCount );
		} else {
			iNumColors = 0;
		}

		//-----------------------------------------------------------------------
		// Get the palette from the resource.  
		// Select to the DC and realize it in the System palette.
		//-----------------------------------------------------------------------
//		*lphPalette = CreateDIBPalette((LPBITMAPINFO) lpbi, &iNumColors );
		if ( lphPalette != NULL ) {
			SelectPalette( hdc, lphPalette, FALSE );
			RealizePalette( hdc );
		}

		//-----------------------------------------------------------------------
		// Now create the bitmap itself.
		//-----------------------------------------------------------------------
		hBitmapFinal = CreateDIBitmap( 
						hdc, 
						(LPBITMAPINFOHEADER)lpbi,
						(LONG)CBM_INIT,
						(LPTSTR)lpbi + lpbi->biSize + iNumColors * sizeof( RGBQUAD ),
						(LPBITMAPINFO)lpbi,
						DIB_RGB_COLORS );

		//-----------------------------------------------------------------------
		// Free DS and memory used.
		//-----------------------------------------------------------------------
		ReleaseDC( NULL, hdc );                        
		UnlockResource( hGlobal );
		FreeResource( hGlobal );
	}
	return( hBitmapFinal );
}

//*****************************************************************************
// Cant_Find_MessageBox -- Find & Load the bitmap from the resource.
//                                                                 
// INPUT:   HINSTANCE hInstance -- Program's hInstance.
//          LPTSTR lpString -- name of bitmap to find.
//          HPALETTE FAR *lphPalette -- we will return palette in this.
//
// OUTPUT:  HBITMAP -- handle to the bitmap if found.
//                                                                 
// WARNINGS:
//                                                                 
// HISTORY: Found this routine on MS Developmemt CD, July 1996.
// 	09/26/1996  MML : Created.  
//  08/27/2003  JFS : Repaired!                                          
//=============================================================================

void Cant_Find_MessageBox ( HINSTANCE hInstance, char *szPath )
{

	//
	// JFS... Added functionality to make this work in wide characters.
	//
#ifdef LEAN_AND_MEAN
	{
		Locale_GetString( "Autorun:AutorunTitle", szWideBuffer );
		swprintf( szWideBuffer3, szWideBuffer, szProductName );

		Locale_GetString( "Autorun:CantFind", szWideBuffer );
		MultiByteToWideChar( CP_ACP, MB_PRECOMPOSED, szPath, _MAX_PATH, szWideBuffer0, _MAX_PATH );
		swprintf( szWideBuffer2, szWideBuffer, szWideBuffer0 );	

		MessageBoxW( NULL,  szWideBuffer2, szWideBuffer3, MB_APPLMODAL | MB_OK );
	}

#else

	std::wstring wideBuffer = TheGameText->fetch("Autorun:AutorunTitle");
	std::wstring wideBuffer2.format( wideBuffer.str(), productName.str() );
	std::wstring wideBuffer3 = TheGameText->fetch("Autorun:CantFind");

	WideCharToMultiByte( CodePage, 0, wideBuffer3.str(), wideBuffer3.getLength()+1, szBuffer3, _MAX_PATH, NULL, NULL );
	WideCharToMultiByte( CodePage, 0, wideBuffer2.str(), wideBuffer2.getLength()+1, szBuffer2, _MAX_PATH, NULL, NULL );


	sprintf( szBuffer1, szBuffer3, szPath );


	if ( strlen( szPath ) < 3 )
	{
		MessageBox( NULL, "The path specified in Cant_Find_MessageBox was blank", "Autorun", MB_APPLMODAL | MB_OK );
		return;
	}	
	if ( strlen( szBuffer1 ) < 3 && strlen( szBuffer3 ) < 3 )
	{
		MessageBox( NULL, "***MISSING MESSAGES***... IDS_AUTORUN_TITLE and IDS_CANT_FIND", "Autorun", MB_APPLMODAL | MB_OK );
		return;
	}	
	if ( strlen( szBuffer1 ) < 3 )
	{
		MessageBox( NULL, "***MISSING MESSAGE***... IDS_AUTORUN_TITLE", "Autorun", MB_APPLMODAL | MB_OK );
		return;
	}	
	if ( strlen( szBuffer3 ) < 3 )
	{
		MessageBox( NULL, "***MISSING MESSAGE***... IDS_CANT_FIND", "Autorun", MB_APPLMODAL | MB_OK );
		return;
	}




	MessageBox( NULL, szBuffer1, szBuffer2, MB_APPLMODAL | MB_OK );
#endif

}


/****************************************************************************** 
 * Error_Message -- 														  
 *                                                                            
 * INPUT:		 															  
 *                                                                            
 * OUTPUT:      
 *                                                                            
 * WARNINGS:   none                                                           
 *                                                                            
 * HISTORY:                                                                   
 *   08/14/1998 MML : Created.                                                
 *============================================================================*/

void Error_Message ( HINSTANCE hInstance, const char * title, const char * string, char *path )
{

#ifndef LEAN_AND_MEAN

	wideBuffer2 = TheGameText->fetch(title);
	wideBuffer3 = TheGameText->fetch(string);

	if ( path && ( path[0] != '\0' )) 
	{
		wideBuffer3.format( wideBuffer.str(), path );
	} 
	else 
	{
		wideBuffer3 = wideBuffer;					// insert not provided
	}

	WideCharToMultiByte( CodePage, 0, wideBuffer2.str(), wideBuffer2.getLength()+1, szBuffer2, _MAX_PATH, NULL, NULL );
	WideCharToMultiByte( CodePage, 0, wideBuffer3.str(), wideBuffer3.getLength()+1, szBuffer3, _MAX_PATH, NULL, NULL );

	MessageBox( NULL, szBuffer3, szBuffer2, MB_APPLMODAL | MB_OK );

#endif

	MessageBox( NULL, "ERROR_UNDEFINED", "ERROR_UNDEFINED", MB_APPLMODAL | MB_OK );


}


/****************************************************************************** 
/ Launch Class Object
/******************************************************************************/

LaunchObjectClass::LaunchObjectClass ( char *path, char *args )
{
	memset( szPath, '\0', _MAX_PATH );
	memset( szArgs, '\0', _MAX_PATH );

	if( path != NULL && path[0] != '\0' ) {
		strcpy( szPath, path );
	}
	if( args != NULL && args[0] != '\0' ) {
		strcpy( szArgs, args );
	}
}

void LaunchObjectClass::SetPath ( char *path )
{
	if( path != NULL && path[0] != '\0' ) {
		memset( szPath, '\0', _MAX_PATH );
		strcpy( szPath, path );
	}
}

void LaunchObjectClass::SetArgs ( char *args )
{
	if( args != NULL && args[0] != '\0' ) {
		memset( szArgs, '\0', _MAX_PATH );
		strcpy( szArgs, args );
	}
}

unsigned int LaunchObjectClass::Launch ( void )
{
	char 	filepath	[_MAX_PATH];
	char 	dir			[_MAX_DIR];
	char 	ext			[_MAX_EXT];
	char 	drive		[_MAX_DRIVE];
	char 	file		[_MAX_FNAME];
	char 	lpszComLine [ 127 ];

	PROCESS_INFORMATION processinfo; 
	STARTUPINFO			startupinfo;
	unsigned int		abc = 0;
	unsigned int		result = 0;

	memset( lpszComLine, '\0', 127 );

	//--------------------------------------------------------------------------
	// Split into parts.
	//--------------------------------------------------------------------------
	_splitpath( szPath, drive, dir, file, ext );

	//--------------------------------------------------------------------------
	// Change current path to the correct dir.
	//
	// The _chdrive function changes the current working drive to the drive 
	// specified by drive. The drive parameter uses an integer to specify the 
	// new working drive (1=A, 2=B, and so forth). This function changes only 
	// the working drive; _chdir changes the working directory.
	//--------------------------------------------------------------------------
	_makepath( filepath, drive, dir, NULL, NULL );
	Path_Remove_Back_Slash( filepath );

	abc = (unsigned)( toupper( filepath[0] ) - 'A' + 1 );
	if ( !_chdrive( abc )) {

		//----------------------------------------------------------------------
		// Returns a value of 0 if successful.
		//----------------------------------------------------------------------
		abc = _chdir( filepath );
	}

#if (_DEBUG)

	int cde = _getdrive();
	_getcwd( szBuffer, _MAX_PATH );

	Msg( __LINE__, TEXT(__FILE__), TEXT("Current Working Dir = %d\\%s." ), cde, szBuffer );
#endif

	//--------------------------------------------------------------------------
	// Try to launch the EXE...
	//--------------------------------------------------------------------------
	_stprintf( lpszComLine, _TEXT( "%s %s" ), szPath, szArgs );

	//==========================================================================
	// Setup the call
	//==========================================================================
	memset( &startupinfo, 0, sizeof( STARTUPINFO ));
	startupinfo.cb = sizeof( STARTUPINFO );

	Msg( __LINE__, TEXT(__FILE__), TEXT("About to launch %s." ), lpszComLine );

	result = CreateProcess( 
				szPath,												// address of module name
				lpszComLine, 										// address of command line
				NULL,												// address of process security attributes
				NULL,												// address of thread security attributes
				FALSE,												// new process inherits handles
				FALSE,
				NULL,												// address of new environment block
				NULL,												// address of current directory name
				&startupinfo,										// address of STARTUPINFO
				&processinfo );										// address of PROCESS_INFORMATION

	//--------------------------------------------------------------------------
	// If WinExec returned 0, error occurred.
	//--------------------------------------------------------------------------
	if ( !result ) {

		Msg( __LINE__, TEXT(__FILE__), TEXT("Launch of %s failed." ), lpszComLine );
		_makepath ( filepath, NULL, NULL, file, ext );
		Cant_Find_MessageBox ( Main::hInstance, filepath );
	}
	Msg( __LINE__, TEXT(__FILE__), TEXT("Launch of %s succeeded." ), lpszComLine );

	return( result );
}

void Debug_Date_And_Time_Stamp ( void )
{
	//-------------------------------------------------------------------------
	//	tm_sec	- Seconds after minute (0 – 59)
	//	tm_min	- Minutes after hour (0 – 59)
	//	tm_hour	- Hours after midnight (0 – 23)
	//	tm_mday	- Day of month (1 – 31)
	//	tm_mon	- Month (0 – 11; January = 0)
	//	tm_year	- Year (current year minus 1900)
	//	tm_wday	- Day of week (0 – 6; Sunday = 0)
	//	tm_yday	- Day of year (0 – 365; January 1 = 0)
	//-------------------------------------------------------------------------
	static char *Month_Strings[ 12 ] = {
		"January",
		"February",
		"March",
		"April",
		"May",
		"June",
		"July",
		"August",
		"September",
		"October",
		"November",
		"December"
	};

	static char *Week_Day_Strings[ 7 ] = {
		"Sunday",
		"Monday",
		"Tuesday",
		"Wednesday",
		"Thursday",
		"Friday",
		"Saturday",
	};

	char		ampm[] = "AM";
    time_t		ltime;
    struct tm *	today;

    /*-------------------------------------------------------------------------
	 *Convert to time structure and adjust for PM if necessary. 
	 */
    time( &ltime );
    today = localtime( &ltime );
    if( today->tm_hour > 12 ) {
		strcpy( ampm, "PM" );
		today->tm_hour -= 12;
    }
	if( today->tm_hour == 0 ) {		/* Adjust if midnight hour. */
		today->tm_hour = 12;
	}

	Msg( __LINE__, __FILE__, "%s, %s %d, %d		%d:%d:%d %s", 
		Week_Day_Strings[ today->tm_wday ],
		Month_Strings[ today->tm_mon ],
		today->tm_mday,		
		today->tm_year + 1900,
		today->tm_hour, 
		today->tm_min, 
		today->tm_sec, 
		ampm );

    /*-------------------------------------------------------------------------
	 * Note how pointer addition is used to skip the first 11 
     * characters and printf is used to trim off terminating 
     * characters.
     */
//	Msg( __LINE__, __FILE__, "%s %s\n", asctime( today ), ampm );
}


bool Is_On_CD ( char *volume_name )
{
	char volume_to_match[ MAX_PATH ];

	Reformat_Volume_Name( volume_name, volume_to_match );

	if( _stricmp( szVolumeName, volume_to_match ) == 0 ) {
		return true;
	} else {
		return false;
	}
}

bool Prompt_For_CD ( HWND window_handle, char *volume_name, const char * message1, const char * message2, int *cd_drive )
{
	int drive;

	strcpy( szBuffer, Args->Get_argv( 0 ));
	drive = toupper( szBuffer[0] ) - 'A';
	memset( szBuffer, '\0', MAX_PATH );

	//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
	// This is the correct check for a CD Check.
	//
	// MML: Modified on 10/18/2000 so it would check for all available CD drives.
	//~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
	int result = IDRETRY;

	while( result == IDRETRY ) {

			if ( CD_Volume_Verification( drive, szBuffer, volume_name )) {

				result = IDOK;
				*cd_drive = drive;

			} else {

				CDList.Reset_Index();

				while(( result == IDRETRY ) && ( CDList.Get_Index() < CDList.Get_Number_Of_Drives())) {

					drive = CDList.Get_Next_CD_Drive();

					if ( CD_Volume_Verification( drive, szBuffer, volume_name )) {
						result = IDOK;
						*cd_drive = drive;
					}
				}
			}

			if ( result == IDRETRY ) {
				result = ( Show_Message( window_handle, message1, message2 ));
			}
	}

	if ( result == IDCANCEL ) {
		return( false );
//		return true;
	}

	return( true );
}



int Show_Message ( HWND window_handle, const char * message1, const char * message2 )
{

#ifndef LEAN_AND_MEAN

	UnicodeString	string1;
	UnicodeString	string2;
	WCHAR	szString3[ MAX_PATH ];
	int		result = false;

	string1 = TheGameText->fetch(message1);
	string2 = TheGameText->fetch(message2);

	wcscpy( szString3, string1.str() );
	wcscat( szString3, L" " );
	wcscat( szString3, string2.str() );

	WideCharToMultiByte( CodePage, 0, szString3, _MAX_PATH, szBuffer, _MAX_PATH, NULL, NULL );
	result = MessageBox( window_handle, szBuffer, "Autorun", MB_RETRYCANCEL|MB_APPLMODAL|MB_SETFOREGROUND );

	return( result );

#else

	return ( 0 );

#endif

}


void Reformat_Volume_Name ( char *volume_name, char *new_volume_name )
{
	char temp_volume_name[ MAX_PATH ];

	strcpy( temp_volume_name, volume_name );

	if( WinVersion.Is_Win95()) {
		temp_volume_name[11] = '\0';
	}

	if( new_volume_name != NULL ) {
		strcpy( new_volume_name, temp_volume_name );
	}
}


