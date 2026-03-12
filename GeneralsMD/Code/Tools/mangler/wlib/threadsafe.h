//
// threadsafe.h
//
// If you include this file and call non-threadsafe functions, it'll
//   prevent your program from compiling.  It's meant to be a slap
//   on the wrist in case you forget and call an unsafe function
//   from a threadsafe program.
//

//
// Reminder to self - use sigwait, not signal handlers!
//

#ifdef _REENTRANT

#ifndef THREADSAFE_HEADER
#define THREADSAFE_HEADER

#define strtok		("strtok() is not MT-SAFE!")
#define ascctime	("asctime() is not MT-SAFE!")
// Can't just do ctime, as Windows' objidl.h uses it as a FILETIME thingie
#define ctime(x)	("ctime() is not MT-SAFE!")
#define gmtime		("gmtime() is not MT-SAFE!")
#define localtime	("localtime() is not MT-SAFE!")
#define tzset		("tzset() is not MT-SAFE!")
#define tzsetwall	("tzsetwall() is not MT-SAFE!")
#define readdir		("readdir() is not MT-SAFE!")

#define rand		("rand() is not MT-SAFE!")
#define srand		("srand() is not MT-SAFE!")
#define random		("random() is not MT-SAFE!")
#define srandom		("srandom() is not MT-SAFE!")

#define tmpnam		("tmpnam() is not MT-SAFE!")
#define vfork		("vfork() is not MT-SAFE!")

#define system          ("system() is not MT-SAFE!")
#define popen           ("popen() is not MT-SAFE!")
#define pclose          ("pclose() is not MT-SAFE!")
#define ctermid         ("ctermid() is not MT-SAFE!")
#define getlogin        ("getlogin() is not MT-SAFE!");

#endif  // THREADSAFE_HEADER
#endif  // _REENTRANT
