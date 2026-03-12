// FILE: WindowProc.h /////////////////////////////////////////////////////////
//
// Project:    ImagePacker
//
// File name:  WindowProc.h
//
// Created:    Colin Day, August 2001
//
// Desc:       Dialog procedure header for image packer utility
//
//-----------------------------------------------------------------------------
///////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __WINDOWPROC_H_
#define __WINDOWPROC_H_

// SYSTEM INCLUDES ////////////////////////////////////////////////////////////

// USER INCLUDES //////////////////////////////////////////////////////////////

// FORWARD REFERENCES /////////////////////////////////////////////////////////

// TYPE DEFINES ///////////////////////////////////////////////////////////////

// INLINING ///////////////////////////////////////////////////////////////////

// EXTERNALS //////////////////////////////////////////////////////////////////
extern BOOL CALLBACK ImagePackerProc( HWND hWndDialog, UINT message,
																			WPARAM wParam, LPARAM lParam );

extern HWND MakePreviewDisplay( void );
extern void UpdatePreviewWindow( void );
extern LRESULT CALLBACK PreviewProc( HWND hWnd, UINT message, 
																		 WPARAM wParam, LPARAM lParam );

extern BOOL CALLBACK ImageErrorProc( HWND hWndDialog, UINT message,
																		 WPARAM wParam, LPARAM lParam );

extern BOOL CALLBACK PageErrorProc( HWND hWndDialog, UINT message,
																		WPARAM wParam, LPARAM lParam );

extern BOOL CALLBACK DirectorySelectProc( HWND hWndDialog, UINT message,
																					WPARAM wParam, LPARAM lParam );

#endif // __WINDOWPROC_H_

