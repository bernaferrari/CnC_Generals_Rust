// FILE: CopyProtection.h ////////////////////////////////////////////////////
// Author: Matthew D. Campbell
// Taken From: Denzil Long's code in Tiberian Sun, by way of Yuri's Revenge
//////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef COPYPROTECTION_H
#define COPYPROTECTION_H

// Comment out the following line to disable copy protection checks
#define DO_COPY_PROTECTION

#ifdef DO_COPY_PROTECTION

class CopyProtect
	{
	public:
		static Bool isLauncherRunning(void);
		static Bool notifyLauncher(void);
		static void checkForMessage(UINT message, LPARAM lParam);
		static Bool validate(void);
		static void shutdown(void);

	private:	
		static LPVOID s_protectedData;
	};

#endif // DO_COPY_PROTECTION

#endif // COPYPROTECTION_H
