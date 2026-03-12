#ifndef EXPORTLOG_H
#define EXPORTLOG_H

#include <windows.h>
 
/**
** ExportLog
** This is an interface to the export log dialog.  
*/
class ExportLog
{
public:
	static void Init(HWND parent);
	static void Shutdown(bool wait_for_ok);

   static void	printf(char *, ...);
	static void rprintf(char *, ...);
	static void	updatebar(float position, float total);
};


#endif //EXPORTLOG_H

