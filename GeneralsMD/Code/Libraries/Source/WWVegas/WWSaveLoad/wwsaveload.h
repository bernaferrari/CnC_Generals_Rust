#if defined(_MSC_VER)
#pragma once
#endif

#ifndef WWSAVELOAD_H
#define WWSAVELOAD_H

/**
** WWSaveLoad
** The Init and Shutdown functions should be called once by the App.
*/

class WWSaveLoad
{
public:
	static void		Init(void);
	static void		Shutdown(void);
};


#endif

