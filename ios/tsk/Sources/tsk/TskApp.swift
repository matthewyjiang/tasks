import SwiftUI

#if SWIFT_PACKAGE
import TskCore
#endif

@main
struct TskApp: App {
    var body: some Scene {
        WindowGroup {
            RootView()
        }
    }
}
