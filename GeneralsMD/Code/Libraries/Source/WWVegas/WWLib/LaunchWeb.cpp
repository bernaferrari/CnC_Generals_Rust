/******************************************************************************
*
* FILE
*     $Archive: /Commando/Code/wwlib/LaunchWeb.cpp $
*
* PROGRAMMER
*     Denzil E. Long, Jr.
*     $Author: Denzil_l $
*
* VERSION INFO
*     $Revision: 2 $
*     $Modtime: 6/22/01 4:39p $
*
******************************************************************************/

#include "LaunchWeb.h"
#include <windows.h>
#include <shellapi.h>
#include <stdio.h>
#include <assert.h>

/******************************************************************************
*
* NAME
*     LaunchWebBrowser
*
* DESCRIPTION
*     Launch the default browser to view the specified URL
*
* INPUTS
*     URL      - Website address
*     Wait     - Wait for user to close browser (default = false)
*     Callback - User callback to invoke during wait (default = NULL callback)
*
* RESULT
*     Success - True if successful; otherwise false
*
******************************************************************************/

bool LaunchWebBrowser(const char* url)
	{
	// Just return if no URL specified
	if (!url || (strlen(url) == 0))
		{
		return false;
		}

	// Create a temporary file with HTML content
	char tempPath[MAX_PATH];
	GetWindowsDirectory(tempPath, MAX_PATH);
	
	char filename[MAX_PATH];
	GetTempFileName(tempPath, "WWS", 0, filename);

	char* extPtr = strrchr(filename, '.');
	strcpy(extPtr, ".html");

	HANDLE file = CreateFile(filename, GENERIC_WRITE, 0, NULL, CREATE_ALWAYS,
			FILE_ATTRIBUTE_NORMAL, NULL);

	assert(INVALID_HANDLE_VALUE != file && "Failed to create temporary HTML file.");

	if (INVALID_HANDLE_VALUE == file)
		{
		return false;
		}

	// Write generic contents
	const char* contents = "<title>ViewHTML</title>";
	DWORD written;
	WriteFile(file, contents, strlen(contents), &written, NULL);
	CloseHandle(file);

	// Find the executable that can launch this file
	char exeName[MAX_PATH];
	HINSTANCE hInst = FindExecutable(filename, NULL, exeName);
	assert(((int)hInst > 32) && "Unable to find executable that will display HTML files.");

	// Delete temporary file
	DeleteFile(filename);

	if ((int)hInst <= 32)
		{
		return false;
		}

	// Launch browser with specified URL
	char commandLine[MAX_PATH];
	sprintf(commandLine, "[open] %s", url);

  STARTUPINFO startupInfo;
	memset(&startupInfo, 0, sizeof(startupInfo));
	startupInfo.cb = sizeof(startupInfo);
  
	PROCESS_INFORMATION processInfo;
	BOOL createSuccess = CreateProcess(exeName, commandLine, NULL, NULL, FALSE,
			0, NULL, NULL, &startupInfo, &processInfo);

	assert(createSuccess && "Failed to launch default WebBrowser.");

	return (TRUE == createSuccess);
	}
